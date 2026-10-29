use std::sync::Arc;

use crate::attribute_api::{
    collect_attribute_messages_storage, decoded_vlen_strings_storage, resolve_vlen_bytes_storage,
    Attribute,
};
use crate::btree_v1;
use crate::btree_v2;
use crate::checksum::jenkins_lookup3;
use crate::dataset::Dataset;
use crate::error::{Error, Result};
use crate::fractal_heap::{FractalHeap, FractalHeapDirectBlockCache};
use crate::io::Cursor;
use crate::local_heap::LocalHeap;
use crate::messages::datatype::VarLenKind;
use crate::messages::link::{self, LinkMessage, LinkTarget};
use crate::messages::link_info::LinkInfoMessage;
use crate::messages::symbol_table_msg::SymbolTableMessage;
use crate::messages::HdfMessage;
use crate::storage::Storage;
use crate::FileContext;

/// A group within an HDF5 file.
#[derive(Clone)]
pub struct Group {
    context: Arc<FileContext>,
    pub(crate) name: String,
    pub(crate) address: u64,
    /// Address of the root group's object header, used for resolving soft links.
    pub(crate) root_address: u64,
}

#[derive(Clone)]
struct ChildEntry {
    name: String,
    location: ObjectLocation,
    /// The link's stored creation order, when its group tracks it.
    creation_order: Option<u64>,
}

#[derive(Clone)]
struct ObjectLocation {
    context: Arc<FileContext>,
    address: u64,
    root_address: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ChildObjectKind {
    Group,
    Dataset,
    Other,
}

impl Group {
    /// Create a group from a known object header address.
    pub(crate) fn new(
        context: Arc<FileContext>,
        address: u64,
        name: String,
        root_address: u64,
    ) -> Self {
        Group {
            context,
            name,
            address,
            root_address,
        }
    }

    /// Group name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Object header address of this group within the file.
    pub fn address(&self) -> u64 {
        self.address
    }

    /// Materialize the full file backing this group.
    pub fn file_data(&self) -> Result<crate::storage::StorageBuffer> {
        self.context.full_file_data()
    }

    /// Access the underlying random-access storage backend.
    pub fn storage(&self) -> &dyn Storage {
        self.context.storage.as_ref()
    }

    /// Size of file offsets in bytes.
    pub fn offset_size(&self) -> u8 {
        self.context.superblock.offset_size
    }

    /// Size of file lengths in bytes.
    pub fn length_size(&self) -> u8 {
        self.context.superblock.length_size
    }

    /// Parse (or retrieve from cache) the object header at the given address.
    fn cached_header(&self, addr: u64) -> Result<Arc<crate::object_header::ObjectHeader>> {
        self.context.get_or_parse_header(addr)
    }

    fn local_location(&self, address: u64) -> ObjectLocation {
        ObjectLocation {
            context: self.context.clone(),
            address,
            root_address: self.root_address,
        }
    }

    /// List all child groups.
    pub fn groups(&self) -> Result<Vec<Group>> {
        let (groups, _) = self.resolve_member_objects()?;
        Ok(groups)
    }

    /// List all child members, partitioned into groups and datasets.
    pub fn members(&self) -> Result<(Vec<Group>, Vec<Dataset>)> {
        self.resolve_member_objects()
    }

    fn resolve_member_objects(&self) -> Result<(Vec<Group>, Vec<Dataset>)> {
        let children = self.resolve_children()?;
        let mut groups = Vec::new();
        let mut datasets = Vec::new();
        for child in &children {
            match self.child_object_kind(child)? {
                ChildObjectKind::Group => {
                    groups.push(Group::new(
                        child.location.context.clone(),
                        child.location.address,
                        child.name.clone(),
                        child.location.root_address,
                    ));
                }
                ChildObjectKind::Dataset => {
                    if let Some(dataset) = self.try_open_child_dataset(child)? {
                        datasets.push(dataset);
                    }
                }
                ChildObjectKind::Other => {}
            }
        }
        Ok((groups, datasets))
    }

    /// Get a child group by name.
    pub fn group(&self, name: &str) -> Result<Group> {
        if let Some(child) = self.resolve_child(name)? {
            return match self.child_object_kind(&child)? {
                ChildObjectKind::Group => Ok(Group::new(
                    child.location.context.clone(),
                    child.location.address,
                    child.name.clone(),
                    child.location.root_address,
                )),
                ChildObjectKind::Dataset => Err(Error::GroupNotFound(format!(
                    "'{}' is a dataset, not a group",
                    name
                ))),
                ChildObjectKind::Other => {
                    Err(Error::GroupNotFound(format!("'{}' is not a group", name)))
                }
            };
        }
        Err(Error::GroupNotFound(name.to_string()))
    }

    /// List all child datasets.
    pub fn datasets(&self) -> Result<Vec<Dataset>> {
        let (_, datasets) = self.resolve_member_objects()?;
        Ok(datasets)
    }

    /// Get a child dataset by name.
    pub fn dataset(&self, name: &str) -> Result<Dataset> {
        if let Some(child) = self.resolve_child(name)? {
            if let Some(dataset) = self.try_open_child_dataset(&child)? {
                return Ok(dataset);
            }
            return Err(Error::DatasetNotFound(name.to_string()));
        }
        Err(Error::DatasetNotFound(name.to_string()))
    }

    /// List attributes on this group.
    pub fn attributes(&self) -> Result<Vec<Attribute>> {
        // `cached_header` already resolves shared messages, including those
        // stored in the shared object header message (SOHM) table, so the
        // group attribute path supports the same sharing the dataset path does.
        let header = self.cached_header(self.address)?;
        Ok(collect_attribute_messages_storage(
            &header,
            self.context.storage.as_ref(),
            self.offset_size(),
            self.length_size(),
            Some(self.context.filter_registry.as_ref()),
        )?
        .into_iter()
        .map(|attr| {
            let element_count = attr.dataspace.num_elements().ok();
            let decoded_strings = element_count.and_then(|element_count| {
                decoded_vlen_strings_storage(
                    &attr.datatype,
                    &attr.raw_data,
                    self.context.storage.as_ref(),
                    self.offset_size(),
                    self.length_size(),
                    element_count,
                )
            });
            let raw_data = match &attr.datatype {
                crate::messages::datatype::Datatype::VarLen {
                    base,
                    kind: VarLenKind::String,
                    ..
                } if base.is_vlen_string_base()
                    && matches!(attr.dataspace.num_elements(), Ok(1)) =>
                {
                    resolve_vlen_bytes_storage(
                        &attr.raw_data,
                        self.context.storage.as_ref(),
                        self.offset_size(),
                        self.length_size(),
                    )
                    .unwrap_or_else(|| attr.raw_data.clone())
                }
                _ => attr.raw_data.clone(),
            };
            Attribute {
                name: attr.name,
                datatype: attr.datatype,
                shape: match attr.dataspace.dataspace_type {
                    crate::messages::dataspace::DataspaceType::Scalar => vec![],
                    crate::messages::dataspace::DataspaceType::Null => vec![0],
                    crate::messages::dataspace::DataspaceType::Simple => attr.dataspace.dims,
                },
                raw_data,
                decoded_strings,
            }
        })
        .collect())
    }

    /// Find an attribute by name.
    pub fn attribute(&self, name: &str) -> Result<Attribute> {
        let attrs = self.attributes()?;
        attrs
            .into_iter()
            .find(|a| a.name == name)
            .ok_or_else(|| Error::AttributeNotFound(name.to_string()))
    }

    /// Resolve children from the object header.
    /// Handles both old-style (symbol table) and new-style (link messages) groups.
    fn resolve_children(&self) -> Result<Vec<ChildEntry>> {
        self.resolve_children_with_link_depth(0)
    }

    fn resolve_child(&self, name: &str) -> Result<Option<ChildEntry>> {
        self.resolve_child_with_link_depth(name, 0)
    }

    fn resolve_child_with_link_depth(
        &self,
        name: &str,
        link_depth: u32,
    ) -> Result<Option<ChildEntry>> {
        let header = self.cached_header(self.address)?;

        let mut link_info: Option<LinkInfoMessage> = None;
        let mut matching_compact_link: Option<LinkMessage> = None;

        for msg in &header.messages {
            match msg {
                HdfMessage::SymbolTable(st) => {
                    return Ok(self
                        .resolve_old_style_group_storage(st)?
                        .into_iter()
                        .find(|child| child.name == name));
                }
                HdfMessage::Link(link) if link.name == name => {
                    matching_compact_link = Some(link.clone());
                }
                HdfMessage::LinkInfo(li) => {
                    link_info = Some(li.clone());
                }
                _ => {}
            }
        }

        if let Some(link) = matching_compact_link {
            if let Some(child) = self.resolve_link_message_target(&link, link_depth)? {
                return Ok(Some(child));
            }
        }

        if let Some(ref li) = link_info {
            if !Cursor::is_undefined_offset(li.fractal_heap_address, self.offset_size()) {
                return self.resolve_dense_link_storage(li, name, link_depth);
            }
        }

        Ok(None)
    }

    /// Resolve children with a soft-link depth counter to prevent cycles.
    fn resolve_children_with_link_depth(&self, link_depth: u32) -> Result<Vec<ChildEntry>> {
        let header = self.cached_header(self.address)?;

        let mut children = Vec::new();

        // Check for old-style groups (symbol table message)
        let mut found_symbol_table = false;
        // Check for new-style groups (link messages)
        let mut link_info: Option<LinkInfoMessage> = None;
        let mut links: Vec<LinkMessage> = Vec::new();

        for msg in &header.messages {
            match msg {
                HdfMessage::SymbolTable(st) => {
                    found_symbol_table = true;
                    children = self.resolve_old_style_group_storage(st)?;
                }
                HdfMessage::Link(link) => {
                    links.push(link.clone());
                }
                HdfMessage::LinkInfo(li) => {
                    link_info = Some(li.clone());
                }
                _ => {}
            }
        }

        if !found_symbol_table {
            // New-style group: use compact links from header messages
            self.resolve_link_targets(&links, link_depth, &mut children)?;

            // Dense-link storage can coexist with compact links, so merge both.
            if let Some(ref li) = link_info {
                if !Cursor::is_undefined_offset(li.fractal_heap_address, self.offset_size()) {
                    for child in self.resolve_dense_links_storage(li, link_depth)? {
                        let is_duplicate = children.iter().any(|existing| {
                            existing.name == child.name
                                && existing.location.address == child.location.address
                                && Arc::ptr_eq(&existing.location.context, &child.location.context)
                        });
                        if !is_duplicate {
                            children.push(child);
                        }
                    }
                }
            }

            // Storage order is not creation order: libhdf5 places compact link
            // messages wherever the object header has room, and dense links
            // sit in name-hash order. When the group tracks creation order
            // (every link carries one, as in all NetCDF-4 groups), list the
            // children in it, as `H5Literate(H5_INDEX_CRT_ORDER)` and netcdf-c
            // do. A stable sort keeps any equal values in storage order.
            if children.iter().all(|child| child.creation_order.is_some()) {
                children.sort_by_key(|child| child.creation_order);
            }
        }

        Ok(children)
    }

    /// Resolve link targets (hard and soft), appending to `children`.
    fn resolve_link_targets(
        &self,
        links: &[LinkMessage],
        link_depth: u32,
        children: &mut Vec<ChildEntry>,
    ) -> Result<()> {
        for link in links {
            if let Some(child) = self.resolve_link_message_target(link, link_depth)? {
                children.push(child);
            }
        }
        Ok(())
    }

    fn resolve_link_message_target(
        &self,
        link: &LinkMessage,
        link_depth: u32,
    ) -> Result<Option<ChildEntry>> {
        match &link.target {
            LinkTarget::Hard { address } => Ok(Some(ChildEntry {
                name: link.name.clone(),
                location: self.local_location(*address),
                creation_order: link.creation_order,
            })),
            LinkTarget::Soft { path } => Ok(self
                .resolve_soft_link_depth(path, link_depth)
                .ok()
                .map(|location| ChildEntry {
                    name: link.name.clone(),
                    location,
                    creation_order: link.creation_order,
                })),
            LinkTarget::External { filename, path } => Ok(self
                .resolve_external_link_depth(filename, path, link_depth)?
                .map(|location| ChildEntry {
                    name: link.name.clone(),
                    location,
                    creation_order: link.creation_order,
                })),
        }
    }

    fn resolve_old_style_group_storage(&self, st: &SymbolTableMessage) -> Result<Vec<ChildEntry>> {
        let heap = LocalHeap::parse_at_storage(
            self.context.storage.as_ref(),
            st.heap_address,
            self.offset_size(),
            self.length_size(),
        )?;

        let leaves = btree_v1::collect_btree_v1_leaves_storage(
            self.context.storage.as_ref(),
            st.btree_address,
            self.offset_size(),
            self.length_size(),
            None,
            &[],
            None,
        )?;

        let mut children = Vec::new();
        for (_key, snod_address) in &leaves {
            let header_len = 8 + 2 * usize::from(self.offset_size());
            let prefix = self.context.read_range(*snod_address, header_len)?;
            let mut prefix_cursor = Cursor::new(prefix.as_ref());
            let sig = prefix_cursor.read_bytes(4)?;
            if sig != *b"SNOD" {
                return Err(Error::InvalidData(format!(
                    "expected SNOD signature at offset {:#x}",
                    snod_address
                )));
            }
            let version = prefix_cursor.read_u8()?;
            if version != 1 {
                return Err(Error::InvalidData(format!(
                    "unsupported symbol table node version {}",
                    version
                )));
            }
            prefix_cursor.skip(1)?;
            let num_symbols = prefix_cursor.read_u16_le()?;
            let node_len =
                8 + usize::from(num_symbols) * (2 * usize::from(self.offset_size()) + 4 + 4 + 16);
            let bytes = self.context.read_range(*snod_address, node_len)?;
            let mut cursor = Cursor::new(bytes.as_ref());
            let snod = crate::symbol_table::SymbolTableNode::parse(
                &mut cursor,
                self.offset_size(),
                self.length_size(),
            )?;

            for entry in &snod.entries {
                let name =
                    heap.get_string_storage(entry.link_name_offset, self.context.storage.as_ref())?;
                children.push(ChildEntry {
                    name,
                    location: self.local_location(entry.object_header_address),
                    creation_order: None,
                });
            }
        }

        Ok(children)
    }

    fn resolve_dense_links_storage(
        &self,
        link_info: &LinkInfoMessage,
        link_depth: u32,
    ) -> Result<Vec<ChildEntry>> {
        let heap = FractalHeap::parse_at_storage(
            self.context.storage.as_ref(),
            link_info.fractal_heap_address,
            self.offset_size(),
            self.length_size(),
        )?;

        let btree_header = btree_v2::BTreeV2Header::parse_at_storage(
            self.context.storage.as_ref(),
            link_info.btree_name_index_address,
            self.offset_size(),
            self.length_size(),
        )?;

        let records = btree_v2::collect_btree_v2_records_storage(
            self.context.storage.as_ref(),
            &btree_header,
            self.offset_size(),
            self.length_size(),
            None,
            &[],
            None,
        )?;

        let mut children = Vec::new();
        let mut direct_block_cache = FractalHeapDirectBlockCache::default();
        for record in &records {
            let heap_id = match record {
                btree_v2::BTreeV2Record::LinkNameHash { heap_id, .. }
                | btree_v2::BTreeV2Record::CreationOrder { heap_id, .. } => heap_id,
                _ => continue,
            };

            let managed_bytes = heap.get_object_storage_cached_with_registry(
                heap_id,
                self.context.storage.as_ref(),
                self.offset_size(),
                self.length_size(),
                &mut direct_block_cache,
                Some(self.context.filter_registry.as_ref()),
            )?;

            let mut link_cursor = Cursor::new(&managed_bytes);
            let link_msg = link::parse(
                &mut link_cursor,
                self.offset_size(),
                self.length_size(),
                managed_bytes.len(),
            )?;

            match &link_msg.target {
                LinkTarget::Hard { address } => {
                    children.push(ChildEntry {
                        name: link_msg.name.clone(),
                        location: self.local_location(*address),
                        creation_order: link_msg.creation_order,
                    });
                }
                LinkTarget::Soft { path } => {
                    if let Ok(location) = self.resolve_soft_link_depth(path, link_depth) {
                        children.push(ChildEntry {
                            name: link_msg.name.clone(),
                            location,
                            creation_order: link_msg.creation_order,
                        });
                    }
                }
                LinkTarget::External { filename, path } => {
                    if let Some(location) =
                        self.resolve_external_link_depth(filename, path, link_depth)?
                    {
                        children.push(ChildEntry {
                            name: link_msg.name.clone(),
                            location,
                            creation_order: link_msg.creation_order,
                        });
                    }
                }
            }
        }

        Ok(children)
    }

    fn resolve_dense_link_storage(
        &self,
        link_info: &LinkInfoMessage,
        name: &str,
        link_depth: u32,
    ) -> Result<Option<ChildEntry>> {
        let heap = FractalHeap::parse_at_storage(
            self.context.storage.as_ref(),
            link_info.fractal_heap_address,
            self.offset_size(),
            self.length_size(),
        )?;

        let btree_header = btree_v2::BTreeV2Header::parse_at_storage(
            self.context.storage.as_ref(),
            link_info.btree_name_index_address,
            self.offset_size(),
            self.length_size(),
        )?;

        let records = btree_v2::collect_btree_v2_link_name_hash_records_storage(
            self.context.storage.as_ref(),
            &btree_header,
            self.offset_size(),
            self.length_size(),
            jenkins_lookup3(name.as_bytes()),
        )?;

        let mut direct_block_cache = FractalHeapDirectBlockCache::default();
        for record in &records {
            let btree_v2::BTreeV2Record::LinkNameHash { heap_id, .. } = record else {
                continue;
            };

            let managed_bytes = heap.get_object_storage_cached_with_registry(
                heap_id,
                self.context.storage.as_ref(),
                self.offset_size(),
                self.length_size(),
                &mut direct_block_cache,
                Some(self.context.filter_registry.as_ref()),
            )?;

            let mut link_cursor = Cursor::new(&managed_bytes);
            let link_msg = link::parse(
                &mut link_cursor,
                self.offset_size(),
                self.length_size(),
                managed_bytes.len(),
            )?;
            if link_msg.name == name {
                return self.resolve_link_message_target(&link_msg, link_depth);
            }
        }

        Ok(None)
    }

    pub fn child_name_by_address(&self, address: u64) -> Result<Option<String>> {
        Ok(self
            .resolve_children()?
            .into_iter()
            .find(|child| child.location.address == address)
            .map(|child| child.name))
    }

    fn child_context(&self, child: &ChildEntry) -> String {
        format!("child '{}' at {:#x}", child.name, child.location.address)
    }

    fn child_object_kind(&self, child: &ChildEntry) -> Result<ChildObjectKind> {
        let header = self
            .cached_child_header(child)
            .map_err(|err| err.with_context(self.child_context(child)))?;

        Ok(classify_child_header(header.as_ref()))
    }

    fn try_open_child_dataset(&self, child: &ChildEntry) -> Result<Option<Dataset>> {
        let header = self
            .cached_child_header(child)
            .map_err(|err| err.with_context(self.child_context(child)))?;

        if classify_child_header(header.as_ref()) != ChildObjectKind::Dataset {
            return Ok(None);
        }

        Dataset::from_parsed_header(
            crate::dataset::DatasetParseContext {
                context: child.location.context.clone(),
            },
            child.location.address,
            child.name.clone(),
            header.as_ref(),
        )
        .map(Some)
        .map_err(|err| err.with_context(self.child_context(child)))
    }

    fn cached_child_header(
        &self,
        child: &ChildEntry,
    ) -> Result<Arc<crate::object_header::ObjectHeader>> {
        child
            .location
            .context
            .get_or_parse_header(child.location.address)
    }

    /// Maximum nesting depth for soft link resolution.
    const MAX_SOFT_LINK_DEPTH: u32 = 16;

    fn resolve_soft_link_depth(&self, path: &str, depth: u32) -> Result<ObjectLocation> {
        self.resolve_path_location(path, depth, "soft link")
    }

    fn resolve_external_link_depth(
        &self,
        filename: &str,
        path: &str,
        depth: u32,
    ) -> Result<Option<ObjectLocation>> {
        if depth >= Self::MAX_SOFT_LINK_DEPTH {
            return Err(Error::Other(format!(
                "external link resolution exceeded maximum depth ({}) at '{}:{}'",
                Self::MAX_SOFT_LINK_DEPTH,
                filename,
                path,
            )));
        }

        let Some(resolver) = self.context.external_link_resolver.as_ref() else {
            return Ok(None);
        };
        let Some(file) = resolver.resolve_external_link(filename)? else {
            return Ok(None);
        };
        let root = file.root_group()?;
        Ok(Some(root.resolve_path_location(
            path,
            depth + 1,
            "external link",
        )?))
    }

    fn resolve_path_location(
        &self,
        path: &str,
        depth: u32,
        link_kind: &str,
    ) -> Result<ObjectLocation> {
        if depth >= Self::MAX_SOFT_LINK_DEPTH {
            return Err(Error::Other(format!(
                "{} resolution exceeded maximum depth ({}) — possible cycle at '{}'",
                link_kind,
                Self::MAX_SOFT_LINK_DEPTH,
                path,
            )));
        }

        let parts: Vec<&str> = path
            .trim_matches('/')
            .split('/')
            .filter(|s| !s.is_empty())
            .collect();

        if parts.is_empty() {
            return Ok(self.local_location(self.root_address));
        }

        let start_addr = if path.starts_with('/') {
            self.root_address
        } else {
            self.address
        };

        let mut current_group = Group::new(
            self.context.clone(),
            start_addr,
            String::new(),
            self.root_address,
        );

        for &part in &parts[..parts.len() - 1] {
            current_group = current_group.group(part)?;
        }

        let target_name = parts[parts.len() - 1];
        if let Some(child) = current_group.resolve_child_with_link_depth(target_name, depth + 1)? {
            return Ok(child.location);
        }

        Err(Error::Other(format!(
            "{} target '{}' not found",
            link_kind, path
        )))
    }
}

fn classify_child_header(header: &crate::object_header::ObjectHeader) -> ChildObjectKind {
    let mut has_dataset_message = false;
    let mut has_attribute_message = false;

    for msg in &header.messages {
        match msg {
            HdfMessage::SymbolTable(_)
            | HdfMessage::Link(_)
            | HdfMessage::LinkInfo(_)
            | HdfMessage::GroupInfo(_) => return ChildObjectKind::Group,
            HdfMessage::Attribute(_) | HdfMessage::AttributeInfo(_) => {
                has_attribute_message = true;
            }
            HdfMessage::Dataspace(_)
            | HdfMessage::DataLayout(_)
            | HdfMessage::FillValue(_)
            | HdfMessage::FilterPipeline(_) => has_dataset_message = true,
            _ => {}
        }
    }

    if has_dataset_message {
        ChildObjectKind::Dataset
    } else if has_attribute_message {
        ChildObjectKind::Group
    } else {
        ChildObjectKind::Other
    }
}

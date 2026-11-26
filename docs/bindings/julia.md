# Julia

`NIRS4ALLFormats` reads every supported file into Julia structs and exports
them, through the [C ABI](capi.md) with `ccall` and JSON3. It needs Julia 1.10
or later. Parsing and writing happen in the Rust library.

## Install

After registration in the General registry:

```julia
using Pkg
Pkg.add("NIRS4ALLFormats")
Pkg.add("nirs4all_formats_capi_jll")   # until the JLL is a declared dependency
```

From source:

```bash
cargo build --release -p nirs4all-formats-capi
export NIRS4ALL_FORMATS_LIBRARY=$PWD/target/release/libnirs4all_formats_capi.so
julia --project=bindings/julia/NIRS4ALLFormats -e 'using Pkg; Pkg.instantiate(); Pkg.test()'
```

## Native library

When `NIRS4ALL_FORMATS_LIBRARY` names a file, that file is loaded. Otherwise the
package loads `nirs4all_formats_capi_jll` when it is a dependency or installed
in the active environment, and fails with a message naming both options.

## Read and access data

```julia
using NIRS4ALLFormats

records = read_records("set.csv")
first_record = records[1]
signal = first_record.signals["signal"]        # each signal owns its axis
signal.axis.values, signal.axis.unit           # signal.axis.kind
signal.values                                  # NaN for missing points
first_record.targets["protein"]                # `nothing` when missing
first_record.metadata["sample_id"]
first_record.provenance.sources[1].sha256

m = signal_matrix(records, "signal")           # axes must be identical
m.values                                       # length(records) × length(m.axis.values)
```

`read_records(name, bytes)` reads in-memory content; the keywords
`single_record`, `rows`, `cols` and `pixels` select image-cube data. `probe(path)`
lists the matching readers and `readers()` the compiled ones.

## Export and convert

```julia
targets = export_targets()                     # targets.formats, targets.software

out = write_records(records, "jcamp-dx"; base_name="set")
foreach(loss -> println(loss["code"], ": ", loss["message"]), out.losses)
for file in out.files
    write(joinpath("exported", file.name), file.bytes)
end

convert_path("spectrum.spc", "opus", "exported"; overwrite=false)
```

The path-to-path function is `convert_path` (not `convert`, which belongs to
Base). Writers never resample or convert units; what a target cannot carry is
refused or listed in `losses` (see [Exports](../EXPORTS.md)). Failures throw
`FormatsError`, whose `status` is a symbol such as `:io` or `:unknown_target`
(see [C ABI errors](capi.md#errors)).

## Publishing

Registration is done by the maintainer; nothing in CI registers a package.

1. **JLL.** Copy `bindings/julia/yggdrasil/N/nirs4all_formats_capi/` to the
   same path in a fork of <https://github.com/JuliaPackaging/Yggdrasil>, set the
   commit hash of the `v<version>` tag in `build_tarballs.jl`, and open a pull
   request. Its CI builds the five platforms; once merged,
   `nirs4all_formats_capi_jll` is registered in General automatically. For later
   releases, update `version` and the hash in another pull request.
2. **Dependency.** Add `nirs4all_formats_capi_jll` to `[deps]` of
   `bindings/julia/NIRS4ALLFormats/Project.toml` with a `[compat]` bound for the
   release series (for example `"0.2.20"` = `>=0.2.20, <0.3`), so `Pkg.add`
   brings the library. The package already looks the JLL up.
3. **TagBot.** `.github/workflows/TagBot.yml` is in place; it needs no secret.
4. **Register.** Install the JuliaRegistrator GitHub app on the repository,
   then comment on the release commit:

   ```text
   @JuliaRegistrator register subdir=bindings/julia/NIRS4ALLFormats
   ```

   After the General pull request merges (automatically for a new package
   after a three-day wait, if AutoMerge passes), TagBot creates the tag
   `NIRS4ALLFormats-v<version>` and its GitHub release.

The name `NIRS4ALLFormats` follows the General naming rules checked by
AutoMerge (it starts with a capital letter, contains lowercase letters,
does not end in `.jl` and has at least five characters). Reviewers may still ask
about the uncommon capitalization; `Nirs4allFormats` would be the fallback,
which needs the directory, module and file renamed together.

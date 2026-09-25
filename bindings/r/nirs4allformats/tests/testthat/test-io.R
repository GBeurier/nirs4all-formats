sample_path <- function(relative) {
  env_root <- Sys.getenv("NIRS4ALL_FORMATS_REPO", unset = "")
  root <- if (nzchar(env_root)) {
    env_root
  } else {
    # Fragile fallback: walk up to a presumed repo root. Its depth depends on
    # where R CMD check creates the .Rcheck tree, so do NOT require it to exist.
    normalizePath(file.path(testthat::test_path(), "../../../../.."), mustWork = FALSE)
  }
  p <- file.path(root, relative)
  # The sample fixtures live in the repo's samples/ tree and are NOT bundled in
  # the package (too large for CRAN). When they are unreachable — an installed /
  # CRAN / off-tree check — skip rather than error. CI sets NIRS4ALL_FORMATS_REPO
  # so the tests actually run against the checked-out samples.
  if (!file.exists(p)) {
    testthat::skip(paste("sample fixture not available off-tree:", relative))
  }
  p
}

test_that("records are loaded through the Rust backend", {
  records <- nirs4allformats_open_records(sample_path("samples/csv_tsv/synthetic_nirs.csv"))

  expect_length(records, 50)
  expect_equal(records[[1]]$provenance$format, "delimited-text")
})

test_that("dataset converts to matrix and data.frame", {
  dataset <- nirs4allformats_open_dataset(sample_path("samples/csv_tsv/synthetic_nirs.csv"))

  expect_s3_class(dataset, "nirs4allformats_dataset")
  expect_equal(dim(as.matrix(dataset)), c(50, 200))
  expect_equal(nrow(as.data.frame(dataset)), 50)
  expect_equal(dataset$sample_ids[[1]], "S000")
  expect_equal(names(dataset$targets), "protein")
  expect_equal(dataset$axis_kind, "wavelength")
  expect_equal(length(dataset$provenance), 50)
  expect_equal(dataset$provenance[[1]]$format, "delimited-text")
  expect_true(nzchar(dataset$provenance[[1]]$sources[[1]]$sha256))
})

test_that("flat dataset rejects mixed spectral identities", {
  records <- nirs4allformats_open_records(
    sample_path("samples/csv_tsv/synthetic_nirs.csv"))
  records <- records[1:2]
  original <- nirs4allformats:::nirs4allformats_dataset_from_records(records)
  expect_equal(dim(original$x), c(2, 200))
  signal <- names(records[[2]]$signals)[[1]]
  wrong_unit <- records
  wrong_unit[[2]]$signals[[signal]]$axis$unit <- "cm-1"
  expect_error(nirs4allformats:::nirs4allformats_dataset_from_records(wrong_unit),
               "different axes or units")
  wrong_kind <- records
  wrong_kind[[2]]$signals[[signal]]$axis$kind <- "wavenumber"
  expect_error(nirs4allformats:::nirs4allformats_dataset_from_records(wrong_kind),
               "different axes or units")
  wrong_type <- records
  wrong_type[[2]]$signals[[signal]]$signal_type <- "reflectance"
  expect_error(nirs4allformats:::nirs4allformats_dataset_from_records(wrong_type),
               "different signal types")
})

test_that("probe_path returns candidate readers", {
  probes <- nirs4allformats_probe_path(sample_path("samples/csv_tsv/synthetic_nirs.csv"))
  expect_true(length(probes) >= 1L)
  expect_equal(probes[[1]]$format, "delimited-text")
})

test_that("walk_path returns parsed entries", {
  entries <- nirs4allformats_walk_path(sample_path("samples/asd"))
  expect_true(length(entries) >= 5L)
  for (entry in entries) {
    expect_equal(entry$status, "parsed")
    expect_equal(entry$format, "asd-fieldspec")
  }
})

test_that("optional large readers are available in the complete build", {
  # This is the complete build: the HDF5, Parquet and MATLAB readers are all
  # compiled in, so these read instead of raising any "not available" error.
  # (The nirs4allformats.lite sibling drops only Parquet.)
  for (relative in c(
    "samples/hdf5/synthetic_nirs.h5",
    "samples/parquet/synthetic_nirs.parquet",
    "samples/matlab/synthetic_nirs_v5.mat"
  )) {
    records <- nirs4allformats_open_records(sample_path(relative))
    expect_true(length(records) >= 1L, label = relative)
  }
})

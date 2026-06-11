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

test_that("optional large readers are available in the full build", {
  # The defining difference from the size-trimmed CRAN package: the HDF5 and
  # Parquet readers (and MATLAB) are compiled in, so these read instead of
  # raising the lite-build "not available" error.
  for (relative in c(
    "samples/hdf5/synthetic_nirs.h5",
    "samples/parquet/synthetic_nirs.parquet"
  )) {
    records <- nirs4allformats_open_records(sample_path(relative))
    expect_true(length(records) >= 1L, label = relative)
  }
})

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

test_that("non-Parquet large readers ARE available in the lite build", {
  # This build drops ONLY the Parquet/Arrow reader; HDF5/netCDF and MATLAB are
  # compiled in, so these read for real instead of raising the excluded-reader
  # error.
  for (relative in c(
    "samples/hdf5/synthetic_nirs.h5",
    "samples/matlab/synthetic_nirs_v5.mat"
  )) {
    records <- nirs4allformats_open_records(sample_path(relative))
    expect_true(length(records) >= 1L, label = relative)
  }
})

test_that("a Parquet input returns the actionable nirs4allformats error", {
  # Parquet is the only excluded reader in the lite build. Feeding one must raise
  # a clean R error naming the complete package nirs4allformats, not the generic
  # "unsupported format" (the graceful-degradation stub).
  path <- sample_path("samples/parquet/synthetic_nirs.parquet")
  err <- tryCatch(
    nirs4allformats_open_records(path),
    error = function(e) conditionMessage(e)
  )
  expect_true(grepl("not available in this build", err, fixed = TRUE))
  expect_true(grepl("nirs4allformats", err, fixed = TRUE))
})

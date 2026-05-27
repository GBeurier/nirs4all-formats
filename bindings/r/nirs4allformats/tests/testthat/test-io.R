sample_path <- function(relative) {
  env_root <- Sys.getenv("NIRS4ALL_FORMATS_REPO", unset = "")
  if (nzchar(env_root)) {
    return(file.path(env_root, relative))
  }
  file.path(normalizePath(file.path(testthat::test_path(), "../../../../..")), relative)
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

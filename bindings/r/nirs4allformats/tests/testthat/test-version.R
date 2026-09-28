test_that("version is exposed", {
  expect_equal(nirs4allformats_version(), as.character(utils::packageVersion("nirs4allformats")))
})

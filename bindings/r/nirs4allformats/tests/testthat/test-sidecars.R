sample_path <- function(relative) {
  env_root <- Sys.getenv("NIRS4ALL_FORMATS_REPO", unset = "")
  if (nzchar(env_root)) {
    return(file.path(env_root, relative))
  }
  file.path(normalizePath(file.path(testthat::test_path(), "../../../../..")), relative)
}

read_bytes <- function(path) {
  readBin(path, what = "raw", n = file.info(path)$size)
}

test_that("ENVI Standard cube decodes from in-memory sidecars", {
  skip_if_not(
    nirs4allformats_native_available(),
    "nirs4allformats_open_with_sidecars requires the extendr static library; install via `R CMD INSTALL bindings/r/nirs4allformats`."
  )
  primary <- sample_path("samples/envi_sli/cubescope-mini-cube.img")
  hdr <- sample_path("samples/envi_sli/cubescope-mini-cube.hdr")
  skip_if(!file.exists(primary), sprintf("missing fixture: %s", primary))
  skip_if(!file.exists(hdr), sprintf("missing fixture: %s", hdr))

  bytes <- read_bytes(primary)
  sidecars <- list("cubescope-mini-cube.hdr" = read_bytes(hdr))
  records <- nirs4allformats_open_with_sidecars(
    "cubescope-mini-cube.img",
    bytes,
    sidecars
  )
  expect_true(length(records) > 0)
  expect_equal(records[[1]]$provenance$format, "envi-standard-cube")
})

test_that("ERDAS LAN decodes from in-memory sidecars", {
  skip_if_not(
    nirs4allformats_native_available(),
    "nirs4allformats_open_with_sidecars requires the extendr static library; install via `R CMD INSTALL bindings/r/nirs4allformats`."
  )
  primary <- sample_path("samples/hyperspectral_cubes/92AV3C.lan")
  spc <- sample_path("samples/hyperspectral_cubes/92AV3C.spc")
  gis <- sample_path("samples/hyperspectral_cubes/92AV3GT.GIS")
  skip_if(!file.exists(primary), sprintf("missing fixture: %s", primary))
  skip_if(!file.exists(spc), sprintf("missing fixture: %s", spc))

  bytes <- read_bytes(primary)
  sidecars <- list(
    "92AV3C.spc" = read_bytes(spc),
    "92AV3GT.GIS" = read_bytes(gis)
  )
  records <- nirs4allformats_open_with_sidecars("92AV3C.lan", bytes, sidecars)
  expect_equal(length(records), 145L * 145L)
})

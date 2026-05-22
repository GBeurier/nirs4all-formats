#!/usr/bin/env Rscript
# Dump a Spectral Evolution `.sed` file via spectrolab as JSON on stdout.
# Usage: Rscript sed_dump.R <path>

args <- commandArgs(trailingOnly = TRUE)
if (length(args) != 1L) {
  stop("usage: sed_dump.R <path>")
}
suppressPackageStartupMessages({
  library(spectrolab)
  library(jsonlite)
})

spectra <- read_spectra(args[[1]], format = "sed")
mat <- as.matrix(spectra)
wls <- as.numeric(colnames(mat))
values <- as.numeric(mat[1L, ])

payload <- list(
  axis = wls,
  values = values
)
cat(toJSON(payload, auto_unbox = TRUE, digits = NA))

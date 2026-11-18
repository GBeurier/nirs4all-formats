#!/usr/bin/env Rscript
# Invoke the separately supplied GPL aquap2 reader in this subprocess only.
args <- commandArgs(trailingOnly = TRUE)
if (length(args) != 3L) {
  stop("usage: nsas_aquap2_dump.R <pinned-source> <input-da> <output-prefix>")
}
# A fixed display zone preserves the six local-clock fields. It does not assert
# that the original acquisition took place in UTC.
Sys.setenv(TZ = "UTC")
eval(parse(file = args[[1]])[[1]])
out <- getNIRData_Vision_da(args[[2]])
writeBin(as.double(t(out$NIR)), paste0(args[[3]], ".f64"), size = 8, endian = "little")
writeLines(substring(colnames(out$NIR), 2), paste0(args[[3]], ".axis.txt"))
writeLines(format(out$timestamp$Timestamp, "%Y-%m-%dT%H:%M:%S"),
           paste0(args[[3]], ".timestamps.txt"))

/*
 * C smoke test for the nirs4all-formats C ABI, compiled and run by
 * tests/c_smoke.rs against the cdylib of the current build:
 *
 *   smoke <path-to-a-JCAMP-DX-file> <existing-output-directory>
 */

#include <stdio.h>
#include <string.h>

#include "nirs4all_formats.h"

static int failures = 0;

static void check(int condition, const char *what) {
    if (!condition) {
        fprintf(stderr, "FAILED: %s\n", what);
        failures++;
    }
}

int main(int argc, char **argv) {
    if (argc != 3) {
        fprintf(stderr, "usage: smoke <jcamp-file> <output-dir>\n");
        return 2;
    }
    char *abi = n4fmt_abi_version();
    char *library = n4fmt_library_version();
    printf("C ABI %s, library %s\n", abi, library);
    check(strcmp(abi, "0.2.0") == 0, "ABI version is 0.2.0");
    n4fmt_string_free(abi);
    n4fmt_string_free(library);

    char *json = NULL;
    char *error = NULL;

    n4fmt_status_t status = n4fmt_reader_catalog(&json, &error);
    check(status == N4FMT_STATUS_OK && strstr(json, "readers::jcamp\"") != NULL,
          "reader catalog lists the JCAMP-DX reader");
    n4fmt_string_free(json);

    status = n4fmt_open_path(argv[1], "{\"single_record\": false}", &json, &error);
    check(status == N4FMT_STATUS_OK && error == NULL, "open_path succeeds");
    check(json != NULL && json[0] == '[', "records are a JSON array");
    char *records = json;

    status = n4fmt_write_records(records, "csv", NULL, &json, &error);
    check(status == N4FMT_STATUS_OK && strstr(json, "\"bytes_base64\"") != NULL,
          "write_records returns base64 files");
    n4fmt_string_free(json);

    status = n4fmt_plan_conversion(records, "winisi", NULL, &json, &error);
    check(status == N4FMT_STATUS_OK && strstr(json, "\"status\"") != NULL,
          "plan_conversion returns a plan");
    n4fmt_string_free(json);

    status = n4fmt_plan_conversion(records, NULL, "{\"signal_type\": \"absorbance\"}", &json, &error);
    check(status == N4FMT_STATUS_OK, "plan_conversion accepts a null target");
    n4fmt_string_free(json);

    status = n4fmt_plan_conversion(records, "no-such-target", NULL, &json, &error);
    check(status == N4FMT_STATUS_UNKNOWN_TARGET && json == NULL && error != NULL,
          "plan_conversion reports an unknown target");
    n4fmt_string_free(error);
    n4fmt_string_free(records);

    status = n4fmt_convert_path(argv[1], "jcamp-dx", argv[2],
                                "{\"base_name\": \"smoke\", \"overwrite\": true}", &json, &error);
    check(status == N4FMT_STATUS_OK && strstr(json, "smoke.jdx") != NULL,
          "convert_path saves smoke.jdx");
    n4fmt_string_free(json);

    status = n4fmt_export_targets(&json, &error);
    check(status == N4FMT_STATUS_OK && strstr(json, "\"software\"") != NULL &&
              strstr(json, "\"requirements\"") != NULL,
          "export targets list software presets and requirements");
    n4fmt_string_free(json);

    status = n4fmt_open_path("/nonexistent/spectrum.dx", NULL, &json, &error);
    check(status == N4FMT_STATUS_IO && json == NULL && error != NULL,
          "a missing file reports N4FMT_STATUS_IO with a message");
    n4fmt_string_free(error);

    static const unsigned char head[] = "##TITLE=x\n##JCAMP-DX=5.01\n";
    status = n4fmt_probe_bytes("x.txt", head, sizeof head - 1, &json, NULL);
    check(status == N4FMT_STATUS_OK && strstr(json, "jcamp-dx") != NULL,
          "probe_bytes detects JCAMP-DX");
    n4fmt_string_free(json);

    if (failures == 0) {
        printf("C smoke: all checks passed\n");
    }
    return failures == 0 ? 0 : 1;
}

using NIRS4ALLFormats
using Test

const SAMPLES = get(ENV, "NIRS4ALL_FORMATS_SAMPLES", normpath(joinpath(@__DIR__, "..", "..", "..", "..", "samples")))
const JCAMP = joinpath(SAMPLES, "jcamp_dx", "TESTSPEC.DX")
const CSV = joinpath(SAMPLES, "csv_tsv", "synthetic_nirs.csv")
const SPC = joinpath(SAMPLES, "galactic_spc", "nir.spc")

@testset "NIRS4ALLFormats" begin
    @testset "versions and readers" begin
        @test abi_version() == "0.2.0"
        @test occursin(r"^\d+\.\d+\.\d+", library_version())
        @test "nirs4all_formats::readers::jcamp" in readers()
    end

    @testset "probe" begin
        @test probe(JCAMP)[1].format == "jcamp-dx"
        @test probe("TESTSPEC.DX", read(JCAMP))[1].format == "jcamp-dx"
    end

    @testset "read JCAMP-DX" begin
        records = read_records(JCAMP)
        @test length(records) == 1
        record = only(records)
        signal = record.signals["signal"]
        @test signal.dims == ["x"]
        @test length(signal.axis.values) == length(signal.values)
        @test signal.axis.kind == "frequency"
        @test record.provenance.format == "jcamp-dx"
        @test length(record.provenance.sources[1].sha256) == 64
        @test read_records("TESTSPEC.DX", read(JCAMP))[1].signals["signal"].values == signal.values
    end

    @testset "signal matrix, targets and metadata" begin
        records = read_records(CSV)
        m = signal_matrix(records)
        @test size(m.values) == (length(records), 200)
        @test m.axis.unit == "nm"
        @test records[1].targets["protein"] isa Real
        @test records[1].metadata["sample_id"] == "S000"
        mixed = [records[1], first(read_records(SPC))]
        @test_throws ArgumentError signal_matrix(mixed, first(keys(mixed[2].signals)))
    end

    @testset "round trip through JCAMP-DX" begin
        records = read_records(SPC)
        out = write_records(records, "jcamp-dx"; base_name="nir")
        @test out.format == "jcamp-dx"
        @test length(out.files) == length(records)
        file = out.files[1]
        original = first(values(records[1].signals))
        back = first(values(read_records(file.name, file.bytes)[1].signals))
        scale = maximum(abs, original.values)
        @test maximum(abs.(original.values .- back.values)) <= scale * 1e-6
        @test maximum(abs.(original.axis.values .- back.axis.values)) <= 1e-6
    end

    @testset "round trip through CSV" begin
        records = read_records(CSV)
        out = write_records(records, "csv"; signal="signal")
        file = only(out.files)
        @test file.media_type == "text/csv"
        back = read_records(file.name, file.bytes)
        @test signal_matrix(back).values == signal_matrix(records).values
        @test signal_matrix(back).axis.values == signal_matrix(records).axis.values
        @test [r.targets["protein"] for r in back] == [r.targets["protein"] for r in records]
    end

    @testset "convert into a directory" begin
        mktempdir() do dir
            result = convert_path(JCAMP, "opus", dir)
            @test result.format == "jcamp-dx"
            @test isfile(only(result.files))
            @test length(read_records(only(result.files))) == 1
            err = try
                convert_path(JCAMP, "opus", dir)
            catch e
                e
            end
            @test err isa FormatsError && err.status == :io
            @test convert_path(JCAMP, "opus", dir; overwrite=true).format == "jcamp-dx"
        end
    end

    @testset "export targets" begin
        targets = export_targets()
        @test any(f -> f.format == "jcamp-dx", targets.formats)
        @test any(s -> s.id == "opus" && !s.tested, targets.software)
    end

    @testset "errors" begin
        @test_throws FormatsError read_records("/nonexistent/x.dx")
        err = try
            read_records("mystery.bin", UInt8[1, 2, 3])
        catch e
            e
        end
        @test err.status == :unsupported_format
        err = try
            write_records(read_records(JCAMP), "no-such-target")
        catch e
            e
        end
        @test err.status == :unknown_target
        @test occursin("no-such-target", err.message)
    end
end

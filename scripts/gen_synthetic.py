"""Generate synthetic NIRS-like sample files for formats without open fixtures."""
from pathlib import Path

import numpy as np
import pandas as pd
import h5py
import scipy.io as sio
from scipy.io import netcdf_file
import pyarrow as pa
import pyarrow.parquet as pq

OUT = Path(__file__).resolve().parents[1] / "samples"
np.random.seed(42)

# Realistic NIRS dataset: 50 samples, 200 wavelengths (1100-2500 nm)
n_samples, n_wl = 50, 200
wl = np.linspace(1100, 2500, n_wl)            # nm
# Synthetic absorbance with 3 Gaussian peaks at typical NIR bands
peaks = [(1450, 80, 0.5), (1940, 100, 0.7), (2280, 50, 0.4)]
base = np.zeros(n_wl)
for c, w, h in peaks:
    base += h * np.exp(-((wl - c) ** 2) / (2 * w ** 2))
sample_var = 0.1 * np.random.randn(n_samples, n_wl)
intensity_factor = 0.8 + 0.4 * np.random.rand(n_samples, 1)
X = (base[None, :] * intensity_factor) + sample_var + 0.05 * np.random.randn(n_samples, n_wl)
y_protein = 10 + 5 * X[:, 50] + 0.5 * np.random.randn(n_samples)   # protein content
sample_ids = [f"S{i:03d}" for i in range(n_samples)]

# 1. CSV (semicolon European-style + comma standard)
df = pd.DataFrame(X, columns=[f"{w:.1f}" for w in wl])
df.insert(0, "sample_id", sample_ids)
df.insert(1, "protein", y_protein)
df.to_csv(f"{OUT}/csv_tsv/synthetic_nirs.csv", index=False)
df.to_csv(f"{OUT}/csv_tsv/synthetic_nirs_semicolon.csv", index=False, sep=";")
df.to_csv(f"{OUT}/csv_tsv/synthetic_nirs.tsv", index=False, sep="\t")

# 2. Excel
df.to_excel(f"{OUT}/excel/synthetic_nirs.xlsx", index=False, sheet_name="spectra")

# 3. Parquet (Zstd)
pq.write_table(pa.Table.from_pandas(df), f"{OUT}/parquet/synthetic_nirs.parquet", compression="zstd")

# 4. NumPy npy / npz
np.save(f"{OUT}/numpy/synthetic_nirs_X.npy", X.astype("float32"))
np.savez(f"{OUT}/numpy/synthetic_nirs.npz",
         X=X.astype("float32"), wavelengths=wl, y=y_protein, sample_ids=np.array(sample_ids))

# 5. MATLAB .mat (v5 + v7.3)
sio.savemat(f"{OUT}/matlab/synthetic_nirs_v5.mat",
            {"X": X, "wavelengths": wl, "y": y_protein, "sample_ids": np.array(sample_ids, dtype=object)})
# v7.3 = HDF5-backed MATLAB
with h5py.File(f"{OUT}/matlab/synthetic_nirs_v73.mat", "w") as f:
    f.attrs["MATLAB_class"] = b"struct"
    f.create_dataset("X", data=X.T)   # MATLAB column-major
    f.create_dataset("wavelengths", data=wl)
    f.create_dataset("y", data=y_protein)

# 6. HDF5 generic
reflectance = np.clip(np.power(10.0, -X), 0.0, 1.5)
with h5py.File(f"{OUT}/hdf5/synthetic_nirs.h5", "w") as f:
    f.create_dataset("spectra", data=X.astype("float32"), compression="gzip")
    f.create_dataset("reflectance", data=reflectance.astype("float32"), compression="gzip")
    f.create_dataset("wavelengths", data=wl)
    f.create_dataset("protein", data=y_protein)
    f["spectra"].attrs["units"] = "absorbance"
    f["reflectance"].attrs["units"] = "reflectance"
    f["wavelengths"].attrs["units"] = "nm"
    f.attrs["instrument"] = "synthetic"
    f.attrs["n_samples"] = n_samples

# 7. NetCDF3 (classic, ANDI-like)
nc = netcdf_file(f"{OUT}/netcdf/synthetic_nirs.nc", "w")
nc.history = "Synthetic NIRS dataset for nirs_loader tests"
nc.createDimension("sample", n_samples)
nc.createDimension("wavelength", n_wl)
vX = nc.createVariable("spectra", "f", ("sample", "wavelength"))
vX[:] = X.astype("float32")
vX.units = "absorbance"
vW = nc.createVariable("wavelengths", "f", ("wavelength",))
vW[:] = wl.astype("float32")
vW.units = "nm"
vY = nc.createVariable("protein", "f", ("sample",))
vY[:] = y_protein.astype("float32")
vY.units = "percent"
nc.close()

# 8. ENVI Spectral Library (.sli + .hdr) - manually build it
# Header text + binary float32 spectra (samples × bands)
sli_path = f"{OUT}/envi_sli/synthetic_lib.sli"
hdr_path = f"{OUT}/envi_sli/synthetic_lib.hdr"
X_sli = X.astype("<f4")  # little-endian float32
X_sli.tofile(sli_path)
hdr = f"""ENVI
description = {{ Synthetic NIRS spectral library for nirs_loader testing }}
samples = {n_wl}
lines = {n_samples}
bands = 1
header offset = 0
file type = ENVI Spectral Library
data type = 4
interleave = bsq
byte order = 0
sensor type = Unknown
wavelength units = Nanometers
spectra names = {{
 {','.join(sample_ids)}
}}
wavelength = {{
 {', '.join(f'{w:.2f}' for w in wl)}
}}
"""
with open(hdr_path, "w") as f:
    f.write(hdr)

# 9. Bruker DPT (two-column ASCII, mean spectrum)
mean_spectrum = X.mean(axis=0)
# DPT is wavenumber, decreasing typically. Convert nm -> cm⁻¹.
wn = 1e7 / wl
order = np.argsort(wn)[::-1]
with open(f"{OUT}/bruker_dpt/synthetic.dpt", "w") as f:
    for i in order:
        f.write(f"{wn[i]:.4f}, {mean_spectrum[i]:.6f}\n")

# 10. IDL/ENVI text output (whitespace-separated with header)
with open(f"{OUT}/csv_tsv/idl_envi_output.txt", "w") as f:
    f.write("; ENVI/IDL output - synthetic NIRS\n")
    f.write("; Wavelength " + " ".join(sample_ids[:5]) + "\n")
    for j, w in enumerate(wl):
        f.write(f"{w:.2f}  " + "  ".join(f"{X[i, j]:.4f}" for i in range(5)) + "\n")

# 11. PP Systems UniSpec .SPT mock (no specifics so just a 2-col text)
with open(f"{OUT}/pp_systems/synthetic_unispec.SPT", "w") as f:
    f.write("File: synthetic_unispec.SPT\n")
    f.write("Date: 2026-05-18\n")
    f.write("Notes: synthetic test fixture for nirs_loader\n")
    f.write("Wavelength,DN_white,DN_target,Reflectance\n")
    for j in range(n_wl):
        dn_t = 1000 + int(500 * X[0, j])
        dn_w = 1500
        f.write(f"{wl[j]:.2f},{dn_w},{dn_t},{dn_t/dn_w:.4f}\n")

# 12. Microtops mock TXT
with open(f"{OUT}/microtops/synthetic_microtops.TXT", "w") as f:
    f.write("REC,DATE,TIME,LATITUDE,LONGITUDE,ALTITUDE,PRESSURE,SZA,AM,TEMP,SDCORR,AOT_1020,AOT_870,AOT_675,WATER\n")
    for i in range(20):
        f.write(f"{i+1},05/18/2026,{10+i//4:02d}:{(i*7)%60:02d}:00,48.85,2.35,35,1013,32.5,1.18,22.3,1.024,0.124,0.156,0.211,1.45\n")

# 13. MODTRAN5 albedo .dat (band model)
with open(f"{OUT}/modtran/synthetic_albedo.dat", "w") as f:
    f.write("# MODTRAN5 albedo file - synthetic\n")
    f.write("WAVELENGTH_um  ALBEDO\n")
    for j in range(n_wl):
        f.write(f"{wl[j]*1e-3:.4f}  {0.3 + 0.1*np.sin(wl[j]*0.001):.4f}\n")

# 14. Foss WinISI text export mock (.NIR text export style)
with open(f"{OUT}/foss_winisi/synthetic_winisi_export.txt", "w") as f:
    f.write("WinISI II Calibration Export - synthetic\n")
    f.write("Number of samples: 50\n")
    f.write("Number of wavelengths: 200\n")
    f.write("Wavelength range (nm): 1100-2500\n")
    f.write("\nWavelengths:\n")
    f.write(" ".join(f"{w:.1f}" for w in wl) + "\n\n")
    f.write("Sample_ID Protein " + " ".join(f"P{j+1}" for j in range(n_wl)) + "\n")
    for i in range(n_samples):
        f.write(f"{sample_ids[i]} {y_protein[i]:.3f} " + " ".join(f"{v:.5f}" for v in X[i]) + "\n")

# 15. VIAVI MicroNIR CSV export mock
with open(f"{OUT}/viavi_micronir/synthetic_micronir.csv", "w") as f:
    f.write("Instrument,VIAVI MicroNIR Pro\n")
    f.write("Serial,P-123456\n")
    f.write("Method,synthetic_test\n")
    f.write("Date,2026-05-18\n")
    f.write(",")
    f.write(",".join(f"{w:.1f}" for w in wl) + "\n")
    for i in range(20):
        f.write(f"{sample_ids[i]}," + ",".join(f"{v:.6f}" for v in X[i]) + "\n")

# 16. Si-Ware NeoSpectra CSV export mock
with open(f"{OUT}/siware_neospectra/synthetic_neospectra.csv", "w") as f:
    f.write("# NeoSpectra Scanner export\n")
    f.write("# Site: Lab\n")
    f.write("# Soil moisture (%): 12.5\n")
    f.write("# GPS: 48.85, 2.35\n")
    f.write("Wavelength_nm,Absorbance\n")
    for j in range(n_wl):
        f.write(f"{wl[j]:.2f},{X[0,j]:.6f}\n")

# 17. Metrohm Vision Air CSV mock
with open(f"{OUT}/metrohm/synthetic_visionair.csv", "w") as f:
    f.write("Vision Air Export\n")
    f.write("Sample;Protein;Moisture;Fat;" + ";".join(str(int(w)) for w in wl) + "\n")
    for i in range(n_samples):
        f.write(f"{sample_ids[i]};{y_protein[i]:.2f};{6+np.random.rand()*3:.2f};{1+np.random.rand()*3:.2f};")
        f.write(";".join(f"{v:.5f}" for v in X[i]) + "\n")

# 18. Foss DS3 / Inframatic CSV report mock
with open(f"{OUT}/foss_winisi/synthetic_ds3_report.csv", "w") as f:
    f.write("Instrument,FOSS NIRS DS3\nDate,2026-05-18\nMethod,Wheat NIR\n\n")
    f.write("SampleID,Protein,Moisture,Starch,Fat\n")
    for i in range(20):
        f.write(f"{sample_ids[i]},{y_protein[i]:.2f},{12+np.random.rand()*2:.2f},{60+np.random.rand()*5:.2f},{2+np.random.rand():.2f}\n")

# 19. PP Systems UniSpec .SPU (Dual Channel mock)
with open(f"{OUT}/pp_systems/synthetic_unispec_dc.SPU", "w") as f:
    f.write("File: synthetic_unispec_dc.SPU\n")
    f.write("Date: 2026-05-18\n")
    f.write("Wavelength,Channel_A_DN,Channel_B_DN,Reflectance\n")
    for j in range(n_wl):
        dn_a = 1000 + int(500 * X[0, j])
        dn_b = 800 + int(400 * X[1, j])
        f.write(f"{wl[j]:.2f},{dn_a},{dn_b},{dn_a/(dn_b+1):.4f}\n")

# 20. ASD .ILL companion file mock (binary, mimics layout)
# ASD calibration files are tiny binaries; we won't fake actual format, just place a note.

# 21. AnIML mock (XML)
animl = f"""<?xml version="1.0" encoding="UTF-8"?>
<AnIML xmlns="urn:org:astm:animl:schema:core:draft:0.90">
  <SampleSet>
    <Sample sampleID="S001" name="Synthetic NIRS sample">
      <Category name="Properties">
        <Parameter name="Protein" parameterType="Float">
          <F>{y_protein[0]:.2f}</F>
        </Parameter>
      </Category>
    </Sample>
  </SampleSet>
  <ExperimentStepSet>
    <ExperimentStep experimentStepID="E001" name="NIRS measurement">
      <Result name="Spectrum">
        <SeriesSet name="Spectrum data" length="{n_wl}">
          <Series seriesID="wavelength" name="Wavelength" seriesType="Float" plotScale="linear">
            <IndividualValueSet>
              {''.join(f'<F>{w:.2f}</F>' for w in wl)}
            </IndividualValueSet>
            <Unit label="nm" quantity="length">
              <SIUnit factor="1.0E-9">m</SIUnit>
            </Unit>
          </Series>
          <Series seriesID="absorbance" name="Absorbance" seriesType="Float" plotScale="linear">
            <IndividualValueSet>
              {''.join(f'<F>{v:.5f}</F>' for v in X[0])}
            </IndividualValueSet>
          </Series>
        </SeriesSet>
      </Result>
    </ExperimentStep>
  </ExperimentStepSet>
</AnIML>
"""
with open(f"{OUT}/animl/synthetic_nirs.animl", "w") as f:
    f.write(animl)

# 22. FGI HDF5+XML structure mock
with h5py.File(f"{OUT}/fgi/synthetic_fgi.h5", "w") as f:
    f.attrs["fgi_schema_version"] = "1.0"
    g = f.create_group("Measurement1")
    g.create_dataset("spectra", data=X.astype("float32"))
    g.create_dataset("wavelengths", data=wl.astype("float32"))
    g.attrs["instrument"] = "FGI-mock"
    g.attrs["operator"] = "synthetic"
    g.attrs["timestamp"] = "2026-05-18T12:00:00Z"

with open(f"{OUT}/fgi/synthetic_fgi.xml", "w") as f:
    f.write("""<?xml version="1.0"?>
<FGIMeasurement>
  <Metadata>
    <Instrument>FGI-mock</Instrument>
    <Operator>synthetic</Operator>
    <Date>2026-05-18</Date>
  </Metadata>
  <DataReference path="synthetic_fgi.h5" />
</FGIMeasurement>
""")

# 23. JASCO JWS text export mock
with open(f"{OUT}/jasco/synthetic_jws_export.txt", "w") as f:
    f.write("TITLE\tSynthetic JASCO V-770 NIR\n")
    f.write("DATA TYPE\tABSORBANCE\n")
    f.write("ORIGIN\tJASCO\n")
    f.write("OWNER\tsynthetic\n")
    f.write("DATE\t2026/05/18\n")
    f.write("TIME\t12:00:00\n")
    f.write("SPECTROMETER/DATA SYSTEM\tV-770\n")
    f.write("XUNITS\tNANOMETERS\n")
    f.write("YUNITS\tABSORBANCE\n")
    f.write("FIRSTX\t" + f"{wl[0]:.4f}\n")
    f.write("LASTX\t" + f"{wl[-1]:.4f}\n")
    f.write("NPOINTS\t" + f"{n_wl}\n")
    f.write("XYDATA\n")
    for j in range(n_wl):
        f.write(f"{wl[j]:.4f}\t{X[0,j]:.6f}\n")

# 24. Shimadzu UVProbe text export mock
with open(f"{OUT}/shimadzu/synthetic_uvprobe.txt", "w") as f:
    f.write('"Spectrum Data"\n')
    f.write(f'"Wavelength nm","Sample {sample_ids[0]}"\n')
    for j in range(n_wl):
        f.write(f"{wl[j]:.4f},{X[0,j]:.6f}\n")

print("Synthetic samples generated.")

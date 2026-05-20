from nirs4all_io import NirsDataset


def test_dataset_shape_contract() -> None:
    dataset = NirsDataset(
        x=[[0.1, 0.2], [0.3, 0.4]],
        wavelengths=[1100.0, 1110.0],
        targets={"protein": [12.0, 13.0]},
        sample_ids=["S001", "S002"],
    )

    assert len(dataset.x) == 2
    assert list(dataset.targets) == ["protein"]

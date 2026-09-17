"""Regression tests for the public release-archive policy."""

from __future__ import annotations

import pytest

from scripts.check_release_archive import forbidden_member


@pytest.mark.parametrize(
    "member",
    [
        "release/samples_local/customer.bin",
        "release/Samples_Local/customer.bin",
        r"release\samples_local\customer.bin",
        r"release\SAMPLES_LOCAL\customer.bin",
        "release/private.enc",
        "release/private.ENC",
        "release/private.key",
        "release/private.KEY",
        "release/SAMPLES_LOCAL.TAR.GZ.ENC",
    ],
)
def test_forbidden_member_is_cross_platform_and_case_insensitive(member: str) -> None:
    assert forbidden_member(member)


@pytest.mark.parametrize(
    "member",
    [
        "release/scripts/samples_local_crypt.sh",
        "release/samples/local.enc.txt",
        "release/samples_locality/customer.bin",
        r"release\samples\public.bin",
    ],
)
def test_forbidden_member_does_not_overmatch_public_paths(member: str) -> None:
    assert not forbidden_member(member)

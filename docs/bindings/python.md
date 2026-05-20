# Python Binding Plan

Python bindings are thin wrappers over the Rust core.

Required surfaces:

- raw record access;
- numpy export;
- pandas export;
- sklearn-compatible dataset providers;
- torch dataset adapters;
- wheel builds through GitHub Actions after the first readers are stable.

Parser logic must stay in Rust.

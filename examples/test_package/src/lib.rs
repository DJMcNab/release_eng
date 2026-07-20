// Copyright 2026 the Release Engineering Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! A package used to demonstrate the release engineering in this repository.

/// Returns an arbitrary English language greeting.
pub fn greeting() -> &'static str {
    "Hello, world!"
}

/// Returns an arbitrary greeting in Latin.
pub fn salutatio() -> &'static str {
    "salve Terra!"
}

// Copyright 2023 Google LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! `xgettext` for `mdbook`
//!
//! This program works like `xgettext`, meaning it will extract
//! translatable strings from your book. The strings are saved in a
//! GNU Gettext `messages.pot` file in your build directory (typically
//! `po/messages.pot`). When the `depth` parameter is included, a new
//! directory will contain the template files split based on the tiers
//! of Chapter nesting.

use anyhow::Context;
use mdbook::renderer::{RenderContext, Renderer};
use mdbook_i18n_helpers::renderers::Xgettext;
use std::io;

fn main() -> anyhow::Result<()> {
    let ctx = RenderContext::from_json(&mut io::stdin()).context("Parsing stdin")?;
    let renderer = Xgettext {};
    renderer.render(&ctx)
}

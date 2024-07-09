/* This file is part of DarkFi (https://dark.fi)
 *
 * Copyright (C) 2020-2024 Dyne.org foundation
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU Affero General Public License as
 * published by the Free Software Foundation, either version 3 of the
 * License, or (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU Affero General Public License for more details.
 *
 * You should have received a copy of the GNU Affero General Public License
 * along with this program.  If not, see <https://www.gnu.org/licenses/>.
 */

use colored::Colorize;

pub fn ansi_texture(width: usize, height: usize, data: &Vec<u8>) -> String {
    let mut out = String::new();

    out.push('┌');
    for j in 0..width {
        out.push('─');
    }
    out.push('┐');
    out.push('\n');

    for i in 0..height {
        out.push('│');
        for j in 0..width {
            let idx = 4 * (i * width + j);

            let r = data[idx];
            let g = data[idx + 1];
            let b = data[idx + 2];
            let a = data[idx + 3];

            #[cfg(target_os = "android")]
            {
                if a > 204 {
                    out.push('█');
                } else if a > 153 {
                    out.push('▓');
                } else if a > 102 {
                    out.push('▒');
                } else if a > 51 {
                    out.push('░');
                } else {
                    out.push(' ');
                }
            }

            #[cfg(target_os = "linux")]
            {
                let r = ((a as f32 * r as f32) / 255.) as u8;
                let g = ((a as f32 * g as f32) / 255.) as u8;
                let b = ((a as f32 * b as f32) / 255.) as u8;

                let val = "█".truecolor(r, g, b).to_string();
                out.push_str(&val);
            }
        }
        out.push('│');
        out.push('\n');
    }

    out.push('└');
    for j in 0..width {
        out.push('─');
    }
    out.push('┘');
    out.push('\n');

    out
}

pub struct TupleIterStruct3<I1, I2, I3> {
    idx: usize,
    i1: I1,
    i2: I2,
    i3: I3,
}

impl<I1, I2, I3> Iterator for TupleIterStruct3<I1, I2, I3>
where
    I1: Iterator,
    I2: Iterator,
    I3: Iterator,
{
    type Item = (usize, I1::Item, I2::Item, I3::Item);

    fn next(&mut self) -> Option<Self::Item> {
        let Some(x1) = self.i1.next() else { return None };
        let Some(x2) = self.i2.next() else { return None };
        let Some(x3) = self.i3.next() else { return None };

        let res = (self.idx, x1, x2, x3);
        self.idx += 1;

        Some(res)
    }
}

pub fn zip3<X1, X2, X3, I1, I2, I3>(i1: I1, i2: I2, i3: I3) -> TupleIterStruct3<I1, I2, I3>
where
    I1: Iterator<Item = X1>,
    I2: Iterator<Item = X2>,
    I3: Iterator<Item = X3>,
{
    TupleIterStruct3 { idx: 0, i1, i2, i3 }
}

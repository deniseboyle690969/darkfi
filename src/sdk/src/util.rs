/* This file is part of DarkFi (https://dark.fi)
 *
 * Copyright (C) 2020-2022 Dyne.org foundation
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

use super::error::ContractError;

pub fn set_return_data(data: &[u8]) -> Result<(), ContractError> {
    unsafe {
        return match set_return_data_(data.as_ptr(), data.len() as u32) {
            0 => Ok(()),
            errcode => Err(ContractError::from(errcode)),
        }
    }
}

pub fn put_object_bytes(data: &[u8]) -> i64 {
    unsafe { return put_object_bytes_(data.as_ptr(), data.len() as u32) }
}

pub fn get_object_bytes(data: &mut [u8], object_index: u32) -> i64 {
    unsafe { return get_object_bytes_(data.as_mut_ptr(), object_index as u32) }
}

pub fn get_object_size(object_index: u32) -> i64 {
    unsafe { return get_object_size_(object_index as u32) }
}

extern "C" {
    fn set_return_data_(ptr: *const u8, len: u32) -> i64;
    fn put_object_bytes_(ptr: *const u8, len: u32) -> i64;
    fn get_object_bytes_(ptr: *mut u8, len: u32) -> i64;
    fn get_object_size_(len: u32) -> i64;
}

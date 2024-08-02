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

pub type Result<T> = std::result::Result<T, Error>;

#[repr(u8)]
#[derive(Debug, Copy, Clone, thiserror::Error)]
pub enum Error {
    #[error("Invalid scene path")]
    InvalidScenePath = 1,

    #[error("Node not found")]
    NodeNotFound = 2,

    #[error("Child node not found")]
    ChildNodeNotFound = 3,

    #[error("Parent node not found")]
    ParentNodeNotFound = 4,

    #[error("Property already exists")]
    PropertyAlreadyExists = 5,

    #[error("Property not found")]
    PropertyNotFound = 6,

    #[error("Property has wrong type")]
    PropertyWrongType = 7,

    #[error("Property has wrong subtype")]
    PropertyWrongSubType = 8,

    #[error("Property value has the wrong length")]
    PropertyWrongLen = 9,

    #[error("Property index is wrong")]
    PropertyWrongIndex = 10,

    #[error("Property out of range")]
    PropertyOutOfRange = 11,

    #[error("Property null not allowed")]
    PropertyNullNotAllowed = 12,

    #[error("Property S-exprs not allowed")]
    PropertySExprNotAllowed = 13,

    #[error("Property array is bounded length")]
    PropertyIsBounded = 14,

    #[error("Property enum item is invalid")]
    PropertyWrongEnumItem = 15,

    #[error("Signal already exists")]
    SignalAlreadyExists = 16,

    #[error("Signal not found")]
    SignalNotFound = 17,

    #[error("Slot not found")]
    SlotNotFound = 18,

    #[error("Signal already exists")]
    MethodAlreadyExists = 19,

    #[error("Method not found")]
    MethodNotFound = 20,

    #[error("Nodes are not linked")]
    NodesAreLinked = 21,

    #[error("Nodes are not linked")]
    NodesNotLinked = 22,

    #[error("Node has parents")]
    NodeHasParents = 23,

    #[error("Node has children")]
    NodeHasChildren = 24,

    #[error("Node has a parent with this name")]
    NodeParentNameConflict = 25,

    #[error("Node has a child with this name")]
    NodeChildNameConflict = 26,

    #[error("Node has a sibling with this name")]
    NodeSiblingNameConflict = 27,

    #[error("File not found")]
    FileNotFound = 28,

    #[error("Resource is not found")]
    ResourceNotFound = 29,

    #[error("Python expr eval error")]
    PyEvalErr = 30,

    #[error("Empty S-expr")]
    SExprEmpty = 31,

    #[error("S-expr global not found")]
    SExprGlobalNotFound = 32,

    #[error("Graphics window closed")]
    GfxWindowClosed = 33,

    #[error("Publisher was destroyed")]
    PublisherDestroyed = 34,

    #[error("Empty atlas")]
    AtlasIsEmpty = 35,

    #[error("Channel closed")]
    ChannelClosed = 36,
}

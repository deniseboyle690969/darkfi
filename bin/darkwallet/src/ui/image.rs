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

use async_trait::async_trait;
use image::ImageReader;
use rand::{rngs::OsRng, Rng};
use std::{
    io::Cursor,
    sync::{Arc, Mutex as SyncMutex, Weak},
};

use crate::{
    gfx::{GfxDrawCall, GfxDrawInstruction, GfxDrawMesh, GfxTextureId, Rectangle, RenderApiPtr},
    mesh::{MeshBuilder, MeshInfo, COLOR_WHITE},
    prop::{PropertyPtr, PropertyRect, PropertyStr, PropertyUint32, Role},
    scene::{Pimpl, SceneNodePtr, SceneNodeWeak},
    ExecutorPtr,
};

use super::{DrawUpdate, OnModify, UIObject};

pub type ImagePtr = Arc<Image>;

pub struct Image {
    node: SceneNodeWeak,
    render_api: RenderApiPtr,
    #[allow(dead_code)]
    tasks: Vec<smol::Task<()>>,

    mesh: SyncMutex<Option<MeshInfo>>,
    texture: SyncMutex<Option<GfxTextureId>>,
    dc_key: u64,

    rect: PropertyRect,
    uv: PropertyRect,
    z_index: PropertyUint32,
    path: PropertyStr,

    parent_rect: SyncMutex<Option<Rectangle>>,
}

impl Image {
    pub async fn new(node: SceneNodeWeak, render_api: RenderApiPtr, ex: ExecutorPtr) -> Pimpl {
        debug!(target: "ui::image", "Image::new()");

        let node_ref = &node.upgrade().unwrap();
        let rect = PropertyRect::wrap(node_ref, Role::Internal, "rect").unwrap();
        let uv = PropertyRect::wrap(node_ref, Role::Internal, "uv").unwrap();
        let z_index = PropertyUint32::wrap(node_ref, Role::Internal, "z_index", 0).unwrap();
        let path = PropertyStr::wrap(node_ref, Role::Internal, "path", 0).unwrap();

        let node_name = node_ref.name.clone();
        let node_id = node_ref.id;

        let self_ = Arc::new_cyclic(|me: &Weak<Self>| {
            let mut on_modify = OnModify::new(ex, node_name, node_id, me.clone());
            on_modify.when_change(rect.prop(), Self::redraw);
            on_modify.when_change(uv.prop(), Self::redraw);
            on_modify.when_change(z_index.prop(), Self::redraw);
            on_modify.when_change(path.prop(), Self::reload);

            Self {
                node,
                render_api,
                tasks: on_modify.tasks,

                mesh: SyncMutex::new(None),
                texture: SyncMutex::new(None),
                dc_key: OsRng.gen(),

                rect,
                uv,
                z_index,
                path,

                parent_rect: SyncMutex::new(None),
            }
        });

        *self_.texture.lock().unwrap() = Some(self_.load_texture());

        Pimpl::Image(self_)
    }

    async fn reload(self: Arc<Self>) {
        let texture = self.load_texture();
        let old_texture = std::mem::replace(&mut *self.texture.lock().unwrap(), Some(texture));

        self.clone().redraw().await;

        if let Some(old_texture) = old_texture {
            self.render_api.delete_texture(old_texture);
        }
    }

    fn load_texture(&self) -> GfxTextureId {
        let path = self.path.get();

        // TODO we should NOT use unwrap here
        let data = Arc::new(SyncMutex::new(vec![]));
        let data2 = data.clone();
        miniquad::fs::load_file(&path, move |res| {
            *data2.lock().unwrap() = res.unwrap();
        });
        let data = std::mem::take(&mut *data.lock().unwrap());
        let img =
            ImageReader::new(Cursor::new(data)).with_guessed_format().unwrap().decode().unwrap();
        let img = img.to_rgba8();

        //let img = image::ImageReader::open(path).unwrap().decode().unwrap().to_rgba8();

        let width = img.width() as u16;
        let height = img.height() as u16;
        let bmp = img.into_raw();

        let texture_id = self.render_api.new_texture(width, height, bmp);
        texture_id
    }

    async fn redraw(self: Arc<Self>) {
        let Some(parent_rect) = self.parent_rect.lock().unwrap().clone() else { return };

        let Some(draw_update) = self.get_draw_calls(parent_rect).await else {
            error!(target: "ui::image", "Image failed to draw");
            return;
        };
        self.render_api.replace_draw_calls(draw_update.draw_calls);
        debug!(target: "ui::image", "replace draw calls done");
        assert!(draw_update.freed_textures.is_empty());
        for buff in draw_update.freed_buffers {
            self.render_api.delete_buffer(buff);
        }
    }

    /// Called whenever any property changes.
    fn regen_mesh(&self) -> MeshInfo {
        let rect = self.rect.get();
        let uv = self.uv.get();
        let mesh_rect = Rectangle::from([0., 0., rect.w, rect.h]);
        let mut mesh = MeshBuilder::new();
        mesh.draw_box(&mesh_rect, COLOR_WHITE, &uv);
        mesh.alloc(&self.render_api)
    }

    async fn get_draw_calls(&self, parent_rect: Rectangle) -> Option<DrawUpdate> {
        self.rect.eval(&parent_rect).ok()?;
        let rect = self.rect.get();
        self.uv.eval(&rect).ok()?;

        let mesh = self.regen_mesh();
        let old_mesh = std::mem::replace(&mut *self.mesh.lock().unwrap(), Some(mesh.clone()));

        let texture_id = self.texture.lock().unwrap().expect("Node missing texture_id!");

        // We're finished with these so clean up.
        let mut freed_buffers = vec![];
        if let Some(old) = old_mesh {
            freed_buffers.push(old.vertex_buffer);
            freed_buffers.push(old.index_buffer);
        }

        let mesh = GfxDrawMesh {
            vertex_buffer: mesh.vertex_buffer,
            index_buffer: mesh.index_buffer,
            texture: Some(texture_id),
            num_elements: mesh.num_elements,
        };

        Some(DrawUpdate {
            key: self.dc_key,
            draw_calls: vec![(
                self.dc_key,
                GfxDrawCall {
                    instrs: vec![
                        GfxDrawInstruction::Move(rect.pos()),
                        GfxDrawInstruction::Draw(mesh),
                    ],
                    dcs: vec![],
                    z_index: self.z_index.get(),
                },
            )],
            freed_textures: vec![],
            freed_buffers,
        })
    }
}

#[async_trait]
impl UIObject for Image {
    fn z_index(&self) -> u32 {
        self.z_index.get()
    }

    async fn draw(&self, parent_rect: Rectangle) -> Option<DrawUpdate> {
        debug!(target: "ui::image", "Image::draw()");
        *self.parent_rect.lock().unwrap() = Some(parent_rect);
        self.get_draw_calls(parent_rect).await
    }
}

impl Drop for Image {
    fn drop(&mut self) {
        // TODO: Delete own draw call

        // Free buffers
        // Should this be in drop?
        if let Some(mesh) = &*self.mesh.lock().unwrap() {
            let vertex_buffer = mesh.vertex_buffer;
            let index_buffer = mesh.index_buffer;
            self.render_api.delete_buffer(vertex_buffer);
            self.render_api.delete_buffer(index_buffer);
        }
        let texture_id = self.texture.lock().unwrap().unwrap();
        self.render_api.delete_texture(texture_id);
    }
}

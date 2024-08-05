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

#![feature(deadline_api)]
#![feature(str_split_whitespace_remainder)]
#![feature(duration_millis_float)]

// Use these to incrementally fix warnings with cargo fix
//#![allow(warnings, unused)]
//#![deny(unused_imports)]

use async_lock::Mutex;
use std::sync::{mpsc, Arc};

use darkfi::{
    async_daemonize, cli_desc,
    event_graph::{self, proto::ProtocolEventGraph, EventGraph, EventGraphPtr},
    net::{session::SESSION_DEFAULT, settings::Settings as NetSettings, P2p, P2pPtr},
    rpc::{
        jsonrpc::JsonSubscriber,
        server::{listen_and_serve, RequestHandler},
    },
    system::{sleep, sleep_forever, StoppableTask, StoppableTaskPtr, Subscription},
    util::path::{expand_path, get_config_path},
    Error, Result,
};
use darkfi_serial::{
    async_trait, deserialize_async, AsyncDecodable, SerialDecodable, SerialEncodable,
};

#[macro_use]
extern crate log;
#[allow(unused_imports)]
use log::LevelFilter;

mod app;
mod darkirc;
//mod chatapp;
//mod chatview;
//mod editbox;
mod error;
mod expr;
//mod gfx;
mod gfx2;
mod keysym;
mod mesh;
mod net;
//mod plugin;
mod prop;
mod pubsub;
//mod py;
//mod res;
mod scene;
mod shader;
mod text2;
mod ui;
mod util;

use crate::{net::ZeroMQAdapter, scene::SceneGraph, text2::TextShaper};

pub type ExecutorPtr = Arc<smol::Executor<'static>>;

fn panic_hook(panic_info: &std::panic::PanicInfo) {
    error!("panic occurred: {panic_info}");
    //error!("panic: {}", std::backtrace::Backtrace::force_capture().to_string());
    std::process::exit(1);
}

#[derive(Clone, Debug, SerialEncodable, SerialDecodable)]
pub struct Privmsg {
    pub channel: String,
    pub nick: String,
    pub msg: String,
}

async fn print_evs(ev_sub: Subscription<event_graph::Event>) {
    loop {
        let ev = ev_sub.receive().await;

        // Try to deserialize the `Event`'s content into a `Privmsg`
        let mut privmsg: Privmsg = match deserialize_async(ev.content()).await {
            Ok(v) => v,
            Err(e) => {
                error!("[IRC CLIENT] Failed deserializing incoming Privmsg event: {}", e);
                continue
            }
        };

        info!("ev_id={:?}", ev.id());
        info!("ev: {:?}", ev);
        info!("privmsg: {:?}", privmsg);
        info!("");
    }
}

async fn realmain(ex: ExecutorPtr) -> darkfi::Result<()> {
    let sled_db = sled::open("evgrdb")?;

    let mut p2p_settings: NetSettings = Default::default();
    p2p_settings.app_version = semver::Version::parse("0.5.0").unwrap();
    p2p_settings.seeds.push(url::Url::parse("tcp+tls://lilith1.dark.fi:5262").unwrap());

    let p2p = P2p::new(p2p_settings, ex.clone()).await?;

    let event_graph = EventGraph::new(
        p2p.clone(),
        sled_db.clone(),
        std::path::PathBuf::new(),
        false,
        "darkirc_dag",
        1,
        ex.clone(),
    )
    .await?;

    let prune_task = event_graph.prune_task.get().unwrap();

    info!("Registering EventGraph P2P protocol");
    let event_graph_ = Arc::clone(&event_graph);
    let registry = p2p.protocol_registry();
    registry
        .register(SESSION_DEFAULT, move |channel, _| {
            let event_graph_ = event_graph_.clone();
            async move { ProtocolEventGraph::init(event_graph_, channel).await.unwrap() }
        })
        .await;

    let ev_sub = event_graph.event_pub.clone().subscribe().await;
    let ev_task = ex.spawn(print_evs(ev_sub));

    info!("Starting P2P network");
    p2p.clone().start().await?;

    info!("Waiting for some P2P connections...");
    sleep(5).await;

    // We'll attempt to sync {sync_attempts} times
    let sync_attempts = 4;
    for i in 1..=sync_attempts {
        info!("Syncing event DAG (attempt #{})", i);
        match event_graph.dag_sync().await {
            Ok(()) => break,
            Err(e) => {
                if i == sync_attempts {
                    error!("Failed syncing DAG. Exiting.");
                    p2p.stop().await;
                    return Err(Error::DagSyncFailed)
                } else {
                    // TODO: Maybe at this point we should prune or something?
                    // TODO: Or maybe just tell the user to delete the DAG from FS.
                    error!("Failed syncing DAG ({}), retrying in {}s...", e, 4);
                    sleep(4).await;
                }
            }
        }
    }

    sleep_forever().await;
    // Signal handling for graceful termination.
    //let (signals_handler, signals_task) = SignalHandler::new(ex)?;
    //signals_handler.wait_termination(signals_task).await?;
    info!("Caught termination signal, cleaning up and exiting...");

    info!("Stopping P2P network");
    p2p.stop().await;

    info!("Stopping IRC server");
    prune_task.stop().await;

    info!("Flushing sled database...");
    let flushed_bytes = sled_db.flush_async().await?;
    info!("Flushed {} bytes", flushed_bytes);

    info!("Shut down successfully");

    Ok(())
}

fn newmain() {
    simplelog::TermLogger::init(
        simplelog::LevelFilter::Info,
        simplelog::Config::default(),
        simplelog::TerminalMode::Stdout,
        simplelog::ColorChoice::Auto,
    )
    .unwrap();

    let ex = Arc::new(smol::Executor::new());
    let n_threads = std::thread::available_parallelism().unwrap().get();
    let ex = std::sync::Arc::new(smol::Executor::new());
    let (signal, shutdown) = smol::channel::unbounded::<()>();
    let (_, result) = easy_parallel::Parallel::new()
        // Run four executor threads
        .each(0..n_threads, |_| smol::future::block_on(ex.run(shutdown.recv())))
        // Run the main future on the current thread.
        .finish(|| {
            smol::future::block_on(async {
                realmain(ex.clone()).await?;
                drop(signal);
                Ok::<(), darkfi::Error>(())
            })
        });
}

fn main() {
    // Exit the application on panic right away
    std::panic::set_hook(Box::new(panic_hook));

    #[cfg(target_os = "android")]
    {
        android_logger::init_once(
            android_logger::Config::default().with_max_level(LevelFilter::Debug).with_tag("darkfi"),
        );

        let paths = std::fs::read_dir("/data/data/darkfi.darkwallet/").unwrap();
        for path in paths {
            debug!("{}", path.unwrap().path().display())
        }
    }

    #[cfg(target_os = "linux")]
    {
        // For ANSI colors in the terminal
        colored::control::set_override(true);

        let term_logger = simplelog::TermLogger::new(
            simplelog::LevelFilter::Debug,
            simplelog::Config::default(),
            simplelog::TerminalMode::Mixed,
            simplelog::ColorChoice::Auto,
        );
        simplelog::CombinedLogger::init(vec![term_logger]).expect("logger");
    }

    let ex = Arc::new(smol::Executor::new());
    let sg = Arc::new(Mutex::new(SceneGraph::new()));

    let async_runtime = app::AsyncRuntime::new(ex.clone());
    async_runtime.start();

    let sg2 = sg.clone();
    let ex2 = ex.clone();
    let zmq_task = ex.spawn(async {
        let zmq_rpc = ZeroMQAdapter::new(sg2, ex2).await;
        zmq_rpc.run().await;
    });
    async_runtime.push_task(zmq_task);

    let (method_req, method_rep) = mpsc::channel();
    // The UI actually needs to be running for this to reply back.
    // Otherwise calls will just hang.
    let render_api = gfx2::RenderApi::new(method_req);
    let event_pub = gfx2::GraphicsEventPublisher::new();

    let text_shaper = TextShaper::new();

    let app =
        app::App::new(sg.clone(), ex.clone(), render_api.clone(), event_pub.clone(), text_shaper);
    let app_task = ex.spawn(app.start());
    async_runtime.push_task(app_task);
    //app.clone().start();

    // Nice to see which events exist
    let ev_sub = event_pub.subscribe_key_down();
    let ev_relay_task = ex.spawn(async move {
        debug!(target: "main", "event relayer started");
        loop {
            let Ok((key, mods, repeat)) = ev_sub.receive().await else {
                debug!(target: "main", "Event relayer closed");
                break
            };
            // Ignore keys which get stuck repeating when switching windows
            match key {
                miniquad::KeyCode::LeftShift | miniquad::KeyCode::LeftSuper => continue,
                _ => {}
            }
            if !repeat {
                debug!(target: "main", "key_down event: {:?} {:?} {}", key, mods, repeat);
            }
        }
    });
    async_runtime.push_task(ev_relay_task);
    let ev_sub = event_pub.subscribe_key_up();
    let ev_relay_task = ex.spawn(async move {
        debug!(target: "main", "event relayer started");
        loop {
            let Ok((key, mods)) = ev_sub.receive().await else {
                debug!(target: "main", "Event relayer closed");
                break
            };
            // Ignore keys which get stuck repeating when switching windows
            match key {
                miniquad::KeyCode::LeftShift | miniquad::KeyCode::LeftSuper => continue,
                _ => {}
            }
            debug!(target: "main", "key_up event: {:?} {:?}", key, mods);
        }
    });
    async_runtime.push_task(ev_relay_task);
    let ev_sub = event_pub.subscribe_char();
    let ev_relay_task = ex.spawn(async move {
        debug!(target: "main", "event relayer started");
        loop {
            let Ok((key, mods, repeat)) = ev_sub.receive().await else {
                debug!(target: "main", "Event relayer closed");
                break
            };
            debug!(target: "main", "char event: {:?} {:?} {}", key, mods, repeat);
        }
    });
    async_runtime.push_task(ev_relay_task);

    //let stage = gfx2::Stage::new(method_rep, event_pub);
    gfx2::run_gui(async_runtime, method_rep, event_pub);
    debug!(target: "main", "Started GFX backend");
}

/*
use rustpython_vm::{self as pyvm, convert::ToPyObject};

fn main() {
    let module = pyvm::Interpreter::without_stdlib(Default::default()).enter(|vm| {
        let source = r#"
def foo():
    open("hihi", "w")
    return 110
#max(1 + lw/3, 4*10) + foo(2, True)
"#;
        //let code_obj = vm
        //    .compile(source, pyvm::compiler::Mode::Exec, "<embedded>".to_owned())
        //    .map_err(|err| vm.new_syntax_error(&err, Some(source))).unwrap();
        //code_obj
        pyvm::import::import_source(vm, "lain", source).unwrap()
    });

    fn foo(x: u32, y: bool) -> u32 {
        if y {
            2 * x
        } else {
            x
        }
    }

    let res = pyvm::Interpreter::without_stdlib(Default::default()).enter(|vm| {
        let globals = vm.ctx.new_dict();
        globals.set_item("lw", vm.ctx.new_int(110).to_pyobject(vm), vm).unwrap();
        globals.set_item("lh", vm.ctx.new_int(4).to_pyobject(vm), vm).unwrap();
        globals.set_item("foo", vm.new_function("foo", foo).into(), vm).unwrap();

        let scope = pyvm::scope::Scope::new(None, globals);

        let foo_fn = module.get_attr("foo", vm).unwrap();
        foo_fn.call((), vm).unwrap()

        //vm.run_code_obj(code_obj, scope).unwrap()
    });
    println!("{:?}", res);
}
*/

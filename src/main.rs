use anyhow::{anyhow, Result};
use flatland::Flatland;
use input_window::InputWindow;
use libstardustxr::fusion::client::Client;
use manifest_dir_macros::directory_relative_path;
use std::thread;
use tokio::{runtime::Handle, sync::oneshot};
use winit::{event_loop::EventLoopBuilder, platform::unix::EventLoopBuilderExtUnix};

mod flatland;
mod input_window;
mod panel_ui;

// fn main() {
// 	let (stardust_shutdown_tx, stardust_shutdown_rx) = tokio::sync::oneshot::channel::<()>();
// 	let stardust_shutdown_tx = Arc::new(Mutex::new(Some(stardust_shutdown_tx)));

// 	let event_loop = EventLoop::new();
// 	let proxy = event_loop.create_proxy();
// 	let window = WindowBuilder::new().build(&event_loop).unwrap();
// 	let mut input_window = InputWindow::new(flatland.clone(), window);

// 	event_loop.run(move |event, _, control_flow| {
// 		*control_flow = ControlFlow::Wait;

// 		match event {
// 			Event::WindowEvent { event, .. } => input_window.handle_event(event),
// 			Event::UserEvent(_) => *control_flow = ControlFlow::Exit,
// 			Event::LoopDestroyed => {
// 				stardust_shutdown_tx
// 					.lock()
// 					.take()
// 					.unwrap()
// 					.send(())
// 					.unwrap();
// 				stardust_thread
// 					.lock()
// 					.take()
// 					.unwrap()
// 					.join()
// 					.unwrap()
// 					.unwrap();
// 			}
// 			_ => (),
// 		}
// 	});
// }

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
	let (client, stardust_event_loop) = Client::connect_with_async_loop().await?;
	client
		.set_base_prefixes(&[directory_relative_path!("res")])
		.await?;

	let tokio_handle = Handle::current();
	let flatland = Flatland::new(client).await?;
	let (winit_stop_tx, mut winit_stop_rx) = oneshot::channel::<()>();
	let winit_thread = thread::Builder::new().name("winit".to_owned()).spawn({
		let flatland = flatland.clone();
		move || -> Result<()> {
			let _tokio_guard = tokio_handle.enter();
			let event_loop = EventLoopBuilder::new()
				.with_any_thread(true)
				.with_x11()
				.build();
			let mut input_window = InputWindow::new(&event_loop, flatland)?;

			event_loop.run(move |event, _, control_flow| {
				match winit_stop_rx.try_recv() {
					Ok(_) => {
						control_flow.set_exit();
						return;
					}
					Err(ref e) if *e == oneshot::error::TryRecvError::Closed => {
						return;
					}
					_ => (),
				}

				input_window.handle_event(event);
			});
		}
	})?;

	let result = stardust_event_loop
		.await
		.map_err(|_| anyhow!("Server disconnected"));

	winit_stop_tx
		.send(())
		.expect("Failed to send stop signal to winit thread");
	winit_thread.join().expect("Couldn't rejoin winit thread")?;

	result
}
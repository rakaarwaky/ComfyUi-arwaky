use std::sync::Mutex;
use std::process::Child;
use tauri::Manager;

struct ComfyUiState {
  child: Mutex<Option<Child>>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  let app = tauri::Builder::default()
    .manage(ComfyUiState {
      child: Mutex::new(None),
    })
    .setup(|app| {
      if cfg!(debug_assertions) {
        app.handle().plugin(
          tauri_plugin_log::Builder::default()
            .level(log::LevelFilter::Info)
            .build(),
        )?;
      }

      // Start ComfyUI python backend
      let state = app.state::<ComfyUiState>();
      let current_dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
      
      println!("Tauri current directory: {:?}", current_dir);

      let mut python_path = current_dir.join("venv").join("bin").join("python");
      if !python_path.exists() {
        python_path = std::path::PathBuf::from("/home/raka/App/ComfyUi-arwaky/venv/bin/python");
      }

      let mut comfyui_dir = current_dir.join("ComfyUI");
      if !comfyui_dir.exists() {
        comfyui_dir = std::path::PathBuf::from("/home/raka/App/ComfyUi-arwaky/ComfyUI");
      }

      println!("Spawning ComfyUI using Python path: {:?}", python_path);
      println!("Working directory: {:?}", comfyui_dir);

      match std::process::Command::new(&python_path)
        .arg("main.py")
        .current_dir(&comfyui_dir)
        .env("HIP_VISIBLE_DEVICES", "1")
        .spawn()
      {
        Ok(child) => {
          println!("ComfyUI Python process successfully spawned with PID: {}", child.id());
          *state.child.lock().unwrap() = Some(child);
        }
        Err(err) => {
          eprintln!("Error spawning ComfyUI Python process: {:?}", err);
        }
      }

      // Spawn background thread to poll ComfyUI port and redirect the webview once ready
      let app_handle = app.handle().clone();
      std::thread::spawn(move || {
        let port_addr = "127.0.0.1:8188";
        println!("Background thread started: Polling {}...", port_addr);
        
        loop {
          if std::net::TcpStream::connect(port_addr).is_ok() {
            println!("ComfyUI server is responsive on {}. Redirecting webview window...", port_addr);
            // Extra delay to ensure web server is fully listening
            std::thread::sleep(std::time::Duration::from_millis(500));
            
            if let Some(window) = app_handle.get_webview_window("main") {
              if let Err(e) = window.eval("window.location.href = 'http://127.0.0.1:8188'") {
                eprintln!("Error redirecting webview window: {:?}", e);
              }
            } else {
              eprintln!("Webview window 'main' not found for redirection.");
            }
            break;
          }
          std::thread::sleep(std::time::Duration::from_millis(1000));
        }
      });

      Ok(())
    })
    .build(tauri::generate_context!())
    .expect("error while building tauri application");

  app.run(|_app_handle, event| match event {
    tauri::RunEvent::Exit => {
      let state = _app_handle.state::<ComfyUiState>();
      let mut lock = state.child.lock().unwrap();
      if let Some(mut child) = lock.take() {
        println!("Terminating ComfyUI process with PID: {}", child.id());
        let _ = child.kill();
      }
    }
    _ => {}
  });
}

use ksni::menu::StandardItem;
use ksni::{MenuItem, ToolTip, Tray, TrayService, Status};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

use crate::backend::BackendCommand;

#[derive(Debug, Clone)]
pub enum UiCommand {
    ToggleVisibility,
    Quit,
}

#[derive(Clone)]
pub struct TrayHandle {
    inner: ksni::Handle<YuMicTray>,
}

impl TrayHandle {
    pub fn set_connected(&self) {
        self.inner.update(|tray| {
            *tray.label.lock().unwrap() = "YuMic — Connected".into();
            *tray.icon_name.lock().unwrap() = "audio-input-microphone-symbolic".into();
            *tray.status.lock().unwrap() = Status::Active;
        });
    }

    pub fn set_disconnected(&self) {
        self.inner.update(|tray| {
            *tray.label.lock().unwrap() = "YuMic — Disconnected".into();
            *tray.icon_name.lock().unwrap() = "microphone-sensitivity-muted-symbolic".into();
            *tray.status.lock().unwrap() = Status::Active;
        });
    }


}

pub struct YuMicTray {
    label: Arc<Mutex<String>>,
    icon_name: Arc<Mutex<String>>,
    status: Arc<Mutex<Status>>,
    cmd_tx: Sender<BackendCommand>,
    ui_tx: Sender<UiCommand>,
}

impl YuMicTray {
    pub fn new(cmd_tx: Sender<BackendCommand>, ui_tx: Sender<UiCommand>) -> Self {
        Self {
            label: Arc::new(Mutex::new("YuMic — Disconnected".into())),
            icon_name: Arc::new(Mutex::new("microphone-sensitivity-muted-symbolic".into())),
            status: Arc::new(Mutex::new(Status::Active)),
            cmd_tx,
            ui_tx,
        }
    }

    pub fn start(self) -> TrayHandle {
        let svc = TrayService::new(self);
        let handle = svc.handle();
        svc.spawn();
        TrayHandle { inner: handle }
    }
}

impl Tray for YuMicTray {
    fn id(&self) -> String {
        "io.github.yumic.tray".into()
    }

    fn title(&self) -> String {
        "YuMic".into()
    }

    fn icon_name(&self) -> String {
        self.icon_name.lock().unwrap().clone()
    }

    fn status(&self) -> Status {
        *self.status.lock().unwrap()
    }

    fn tool_tip(&self) -> ToolTip {
        ToolTip {
            title: "YuMic".into(),
            description: self.label.lock().unwrap().clone(),
            ..Default::default()
        }
    }

    // Sol tıkla pencereyi göster/gizle
    fn activate(&mut self, _x: i32, _y: i32) {
        let _ = self.ui_tx.send(UiCommand::ToggleVisibility);
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        let connected = self.label.lock().unwrap().contains("Connected")
            && !self.label.lock().unwrap().contains("Disconnected");

        let mut items = vec![];

        // Göster/Gizle seçeneği
        let ui_tx_show = self.ui_tx.clone();
        items.push(
            StandardItem {
                label: "Show/Hide Window".into(),
                activate: Box::new(move |_| {
                    let _ = ui_tx_show.send(UiCommand::ToggleVisibility);
                }),
                ..Default::default()
            }
            .into(),
        );

        items.push(MenuItem::Separator);

        if connected {
            let cmd_tx = self.cmd_tx.clone();
            items.push(
                StandardItem {
                    label: "Disconnect".into(),
                    activate: Box::new(move |_| {
                        let _ = cmd_tx.send(BackendCommand::Disconnect);
                    }),
                    ..Default::default()
                }
                .into(),
            );
        } else {
            let cmd_tx = self.cmd_tx.clone();
            items.push(
                StandardItem {
                    label: "Connect".into(),
                    activate: Box::new(move |_| {
                        let _ = cmd_tx.send(BackendCommand::Connect {
                            ip: "192.168.1.105".into(),
                            control_port: 8125,
                            media_port: 49152,
                        });
                    }),
                    ..Default::default()
                }
                .into(),
            );
        }

        items.push(MenuItem::Separator);

        let ui_tx_quit = self.ui_tx.clone();
        items.push(
            StandardItem {
                label: "Quit".into(),
                activate: Box::new(move |_| {
                    let _ = ui_tx_quit.send(UiCommand::Quit);
                }),
                ..Default::default()
            }
            .into(),
        );

        items
    }
}

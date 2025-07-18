use std::sync::LazyLock;

use chrono::Locale;
use gettextrs::{self, bind_textdomain_codeset, bindtextdomain, textdomain};

use adw::prelude::*;
use async_channel::{Receiver, Sender};
use gtk::{gdk, gio, glib};
use ksni::blocking::TrayMethods;

use tokio::runtime;
use trayicon::MyTray;

// Crate
mod current_locale;
mod networking;
mod prayer;
mod preferences;
mod rowprayertime;
mod sound;
mod trayicon;
mod window;

use window::MainWindow;

const APP_ID: &str = "io.github.eminfedar.vaktisalah-gtk-rs";
const LOCALIZATION_DOMAIN_NAME: &str = "vaktisalah-gtk-rs";
const LOCALIZATION_PATH: &str = "/app/share/locale";

static RUNTIME: LazyLock<runtime::Runtime> = LazyLock::new(|| {
    println!("Runtime initialized");
    runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .unwrap()
});

static LOCALE: LazyLock<Locale> = LazyLock::new(|| {
    let locale_text = current_locale::current_locale(); // en-US

    match locale_text.as_str() {
        "tr-TR" => Locale::tr_TR,
        "en-US" => Locale::en_US,
        _ => Locale::en_US,
    }
});

#[derive(Debug)]
pub enum TrayMessage {
    Activate,
    Exit,
}

fn main() -> glib::ExitCode {
    setup_localization();

    // Create a new application
    let app = adw::Application::builder().application_id(APP_ID).build();

    app.connect_startup(move |a| {
        load_css();

        let (tx, rx) = async_channel::bounded(2);
        init_tray(tx);
        handle_tray(rx, a.clone());
    });
    app.connect_activate(build_ui);

    // Run the application
    app.run()
}

fn setup_localization() {
    textdomain(LOCALIZATION_DOMAIN_NAME).unwrap();
    bindtextdomain(LOCALIZATION_DOMAIN_NAME, LOCALIZATION_PATH).unwrap();
    bind_textdomain_codeset(LOCALIZATION_DOMAIN_NAME, "UTF-8").unwrap();

    println!("Current locale: {}", *LOCALE);
}

fn load_css() {
    // Load the CSS file and add it to the provider
    let provider = gtk::CssProvider::new();
    provider.load_from_string(include_str!("../data/style.css"));

    // Add the provider to the default screen
    gtk::style_context_add_provider_for_display(
        &gdk::Display::default().expect("Could not connect to a display."),
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}

fn build_ui(app: &adw::Application) {
    // Create new window and present it
    if app.windows().is_empty() {
        let window = MainWindow::new(app);
        window.read_preferences();
        window.init_second_tick();

        window.present();
    } else {
        let windows = app.windows();
        let window: &MainWindow = windows.first().unwrap().dynamic_cast_ref().unwrap();

        window.update_prayer_time_labels();
        window.present();
    }
}

fn handle_tray(receiver: Receiver<TrayMessage>, app: adw::Application) {
    glib::spawn_future_local(async move {
        loop {
            match receiver.recv().await {
                Ok(m) => match m {
                    TrayMessage::Activate => {
                        let window_list = app.windows();
                        let window = window_list.first().unwrap();

                        if window.is_visible() {
                            window.close();
                        } else {
                            app.activate();
                        }
                    }
                    TrayMessage::Exit => app.quit(),
                },
                Err(e) => {
                    eprintln!("error receiving: {e}");
                    break;
                }
            }
        }
    });
}

fn init_tray(sender: Sender<TrayMessage>) {
    gio::spawn_blocking(move || {
        let tray = MyTray { sender };

        match tray.spawn_without_dbus_name() {
            Ok(_) => (),
            Err(e) => eprintln!("Tray Icon failed: {e:#?}"),
        }
    });
}

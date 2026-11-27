//! Daemon D-Bus de ApkStart (`apkstartd`).
//!
//! Fase mínima: expone `org.apkstart.Manager1` en el bus de sesión, devuelve
//! los valores compuestos como JSON embebido en strings y no instala unidades
//! de systemd.

pub mod dbus;
pub mod manager;

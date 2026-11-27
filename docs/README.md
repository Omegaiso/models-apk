# Documentación técnica de models-apk

Este directorio contiene la documentación **técnica** del proyecto. El
[README principal](../README.md) es la carta de presentación; aquí está el
detalle de implementación, dependencias, decisiones y fallos conocidos.

## Mapa de documentos

| Documento | Para qué sirve |
|---|---|
| [arquitectura.md](arquitectura.md) | Componentes, flujo de datos, D-Bus, ciclo de vida del runtime y de las sesiones scrcpy. |
| [estructura.md](estructura.md) | Árbol del repositorio, crates, archivos clave, esquema SQLite, rutas XDG y variables de entorno. |
| [dependencias.md](dependencias.md) | Dependencias Rust y del sistema, versiones mínimas, binarios externos y licencias. |
| [instalacion.md](instalacion.md) | Instalación desde fuente, preparación del SDK/AVD, binfmt, `setup`, desinstalación y diagnóstico de errores. |
| [bugs.md](bugs.md) | Bugs encontrados, estado de cada uno, limitaciones asumidas y riesgos abiertos. |
| [actualizaciones.md](actualizaciones.md) | Diseño de paquetes `.deb`/`.rpm`/Arch, script `install.sh`, GitHub Actions y sistema de actualizaciones (Fase 2). |
| [desarrollo.md](desarrollo.md) | Comandos de compilación, tests, clippy, convenciones y utilidades de captura/QA. |
| [design/settings-ui/](design/settings-ui/) | Laboratorio HTML con las 3 variantes visuales de la UI; la B es la implementada. |
| [plan-inicial/](plan-inicial/) | Plan técnico original: arquitectura, runtime, integración, UX y roadmap. |
| [bitacora/bitacora.md](bitacora/bitacora.md) | Bitácora append-only con decisiones, pruebas y estado real por bloque. |

## Reglas de esta documentación

- **La bitácora es histórica**: no se reescribe; se añade una entrada al cerrar cada bloque.
- **Los bugs no se borran**: se marcan como resueltos con la causa y el commit.
- **Las decisiones con impacto** (motor, densidad, D-Bus, empaquetado) se registran en la
  bitácora y se reflejan en `AGENTS.md` si afectan al desarrollo diario.
- **Las cifras de recursos** son medidas en el host de desarrollo (14 GB de RAM, 12 hilos,
  AMD iGPU + RX 6400, KDE Wayland); no son una promesa para todos los equipos.

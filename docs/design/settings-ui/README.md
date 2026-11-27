# Design Lab — ajustes de models-apk

Prototipo navegable de la UI de ajustes. La variante **B · Centro de control**
ya está implementada en `crates/apkstart-settings` (componentes en `ui.rs`), con
sidebar colapsable y tarjetas que reflotan según el ancho. Este HTML queda como
referencia de diseño y para comparar variantes.

Abrir con:

```bash
xdg-open docs/design/settings-ui/index.html
```

- 9 páginas: Inicio, Aplicaciones, Recursos, Red, Periféricos, Integración,
  Diagnóstico, Acerca de y Resolución.
- 3 variantes por página:
  - **A · Nativo refinado**: evolución directa de la UI actual.
  - **B · Centro de control**: dashboard con estado y métricas arriba.
  - **C · Consola técnica**: densa, monoespaciada, para logs y datos crudos.
- Tema claro/oscuro con preferencia guardada.
- Cada página tiene URL propia: `index.html#aplicaciones`, `#recursos`, etc.
- `#resolucion` incluye una calculadora de dp/dpi para el display virtual de
  scrcpy.

## Regla de densidad

Android dibuja en dp, no en píxeles. Para un display virtual:

```
dpi = round(ancho_ventana_px × 160 / ancho_dp)
```

Con `--flex-display` el dpi queda fijo y el display cambia de píxeles al
redimensionar la ventana: el ancho lógico crece (más columnas), no el zoom.
Valores de referencia con una ventana de 552 px:

| Perfil   | Ancho lógico | DPI | Uso                          |
|----------|--------------|-----|------------------------------|
| Compacto | 360 dp       | 245 | apps con mucho texto         |
| Cómodo   | 411 dp       | 215 | recomendado (Pixel 6)        |
| Tablet   | 600 dp       | 147 | apps adaptativas, más columnas |

Implementado en `apkstart-core`: 552×984 a 215 dpi = 411 dp.

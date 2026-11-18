# Universal AppImage Installer (`uai`)

Aplicación de terminal (TUI) escrita en Rust para **descargar, instalar y desinstalar AppImages** en Linux, integrándolas en el menú de aplicaciones del sistema (lanzador `.desktop` + icono) sin depender de AppImageLauncher.

Nació para instalar el cliente de escritorio de **ClickUp**, pero el catálogo es un archivo JSON editable al que se le pueden agregar más aplicaciones.

---

## Características

- **Descargar** AppImages desde un catálogo JSON (clave/valor) usando `curl`, respetando el nombre de archivo que envía el servidor.
- **Instalar** cualquier `.AppImage` que esté en `~/.applications`:
  - Extrae el AppImage (`--appimage-extract`) en una carpeta temporal para leer su `.desktop` interno **sin ejecutar la aplicación**.
  - Detecta automáticamente nombre, versión, icono, `StartupWMClass` y `MimeType`.
  - Copia el AppImage a un directorio de instalación propio, instala el icono en el tema `hicolor` y crea el lanzador `.desktop`.
  - **Modo avanzado** para personalizar nombre, versión y la limpieza posterior.
- **Actualizar**: lista las aplicaciones instaladas (nombre, versión, fecha, ruta). *Por ahora solo es informativo.*
- **Eliminar**: desinstala una aplicación (AppImage, lanzador e icono) previa confirmación.
- **Ayuda** integrada dentro de la TUI.
- Banner con arcoíris usando `figlet` (opcional).

---

## Requisitos

| Requisito | Uso | ¿Obligatorio? |
|---|---|---|
| Linux | Uso de permisos Unix, rutas XDG y AppImages | Sí |
| Rust **estable más reciente** (fijado en `rust-toolchain.toml`; mínimo 1.85 por la edición 2024) | Compilar el proyecto | Sí (para compilar) |
| `curl` ≥ 7.83 | Descargas (usa la opción `--clobber`) | Sí, para descargar |
| `figlet` con la fuente `pagga` | Banner del título | No (si falta, se muestra un título en negrita) |

> La fuente `pagga` viene normalmente con el paquete `toilet` (p. ej. `sudo apt install figlet toilet`).

---

## Compilación e instalación

El programa no tiene rutas fijas de ninguna máquina: todo se calcula a partir del `$HOME` del usuario que lo ejecuta, y las carpetas que falten se crean solas en el primer arranque. No necesita `sudo`.

```bash
# 1. Instalar Rust si no está disponible (https://rustup.rs)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 2. Dependencias del sistema (ejemplo para Debian/Ubuntu)
sudo apt install curl figlet toilet

# 3. Asegurar la última versión estable de Rust
rustup update stable

# 4. Compilar e instalar el binario en ~/.cargo/bin (queda en el PATH)
cargo install --path .

# 5. Ejecutar
uai
```

El archivo `rust-toolchain.toml` fija el canal `stable` (con `rustfmt` y `clippy`), así que dentro del proyecto rustup siempre usa el toolchain estable instalado, y lo descarga si falta. Para pasar a la última versión estable publicada basta con `rustup update stable`. El proyecto está probado con Rust 1.98.1.

Alternativa sin `cargo install`:

```bash
cargo build --release          # compila con LTO
./target/release/uai           # ejecuta directamente
install -Dm755 target/release/uai ~/.local/bin/uai   # opcional: añadir al PATH
```

También se puede ejecutar directamente con:

```bash
cargo run --release
```

---

## Uso

Al iniciar se muestra el menú principal:

```
Descargar AppImage
Instalar
Actualizar
Eliminar
Ayuda
Salir
```

### Controles

| Tecla | Acción |
|---|---|
| `↑` / `↓` | Mover la selección (o hacer scroll en la ayuda) |
| `Enter` | Confirmar / activar |
| `Esc` | Volver a la pantalla anterior |
| `q` | Salir (desde el menú principal) |
| `d` | Eliminar la app seleccionada (pantalla *Eliminar*) |
| `y` / `n` | Confirmar / cancelar la eliminación |
| Cualquier tecla | Cerrar avisos y la pantalla de resultado de instalación |

### Flujo típico

1. **Descargar AppImage** → seleccionar la app del catálogo → `Enter`. La TUI se oculta temporalmente para mostrar la barra de progreso de `curl` y el archivo queda en `~/.applications`.
2. **Instalar** → elegir el `.AppImage` descargado → `Enter`. Se extrae y se muestra la pantalla de configuración.
3. En la configuración, elegir **Install now** para instalar con los valores detectados, o activar **Advanced mode** para ajustar:

   | Opción | Descripción | Valor por defecto |
   |---|---|---|
   | Custom name | Nombre que aparece en el menú de aplicaciones | Detectado del `.desktop` interno (o nombre del archivo) |
   | Custom version | Versión usada en el nombre del archivo instalado | `X-AppImage-Version` o versión deducida del nombre del archivo (o `unknown`) |
   | Delete temp folder | Borrar la carpeta de extracción tras instalar | **ON** |
   | Delete source .AppImage | Borrar el archivo original de `~/.applications` | **OFF** |

4. La app aparece en el menú de aplicaciones del sistema.

---

## Rutas utilizadas

Las rutas se resuelven con el crate `dirs` (estándar XDG) y se crean automáticamente en el primer arranque.

| Ruta | Contenido |
|---|---|
| `~/.config/universal-appimage-installer/apps.json` | Catálogo de aplicaciones descargables (editable) |
| `~/.local/share/universal-appimage-installer/installed.json` | Registro de aplicaciones instaladas |
| `~/.applications/` | AppImages descargados, listos para instalar |
| `~/.applications/installed/` | AppImages instalados (`<Nombre>-<versión>.AppImage`) |
| `~/.applications/.tmp/install-<fecha>/` | Extracción temporal (se limpia al entrar a *Instalar*) |
| `~/.local/share/applications/<Nombre>.desktop` | Lanzador generado |
| `~/.local/share/icons/hicolor/512x512/apps/` | Icono instalado |

> **¿Por qué `~/.applications` y no `~/Applications`?** AppImageLauncher (si está instalado) vigila `~/Applications` con `appimagelauncherd` e integra automáticamente todo lo que se copia ahí, creando un `.desktop` competidor. La carpeta oculta `~/.applications` no está vigilada, así que se evita esa condición de carrera.
>
> La pantalla *Instalar* solo lista los `.AppImage` que están directamente en `~/.applications`; las copias de `installed/` no aparecen.

---

## Catálogo de aplicaciones

El catálogo por defecto está en [`data/apps.json`](data/apps.json) y se **embebe en el binario** (`include_str!`). En el primer arranque se copia a `~/.config/universal-appimage-installer/apps.json`; a partir de ahí se usa esa copia, por lo que se puede editar libremente.

Formato: un mapa donde la clave es un identificador corto.

```json
{
  "clickup": {
    "name": "ClickUp",
    "category": "Productivity",
    "description": "ClickUp Desktop application for Linux",
    "homepage": "https://clickup.com",
    "download_url": "https://desktop.clickup.com/linux"
  }
}
```

| Campo | Obligatorio | Descripción |
|---|---|---|
| `name` | Sí | Nombre visible |
| `download_url` | Sí | URL de descarga (se siguen redirecciones) |
| `category` | No | Categoría (se muestra `misc` si está vacía) |
| `description` | No | Descripción (si está vacía se muestra `homepage`) |
| `homepage` | No | Página del proyecto |

> Modificar `data/apps.json` solo afecta a instalaciones nuevas. Si el archivo ya existe en `~/.config/...`, hay que editarlo allí (o borrarlo para regenerarlo).

---

## Detalles de la instalación

### Detección de metadatos

Del `.desktop` interno del AppImage se leen `Name`, `X-AppImage-Version`, `Icon`, `StartupWMClass` y `MimeType`. Si no hay versión, se intenta deducir del nombre del archivo (p. ej. `desktop-3.5.343x86_64.AppImage` → `3.5.343`).

### Búsqueda del icono

1. `<Icon>.{png,svg,xpm}` en la raíz de `squashfs-root`.
2. En `usr/share/icons` y `usr/share/pixmaps`, priorizando SVG y luego la resolución más alta.
3. Como último recurso, `.DirIcon`.

El icono se instala con el nombre de `StartupWMClass` en minúsculas (o el nombre de la app). Además, se **sobrescriben los iconos con ese mismo nombre** en los temas locales (`~/.local/share/icons` y `~/.icons`) para que docks o bandejas que resuelven el icono por nombre (ignorando `Icon=<ruta>`) muestren el correcto. Los SVG se reemplazan por un SVG que envuelve el PNG en base64.

### Lanzador generado

```ini
[Desktop Entry]
Type=Application
Name=<nombre>
Comment=Installed with Universal AppImage Installer
Exec="<ruta al AppImage>" %U
TryExec=<ruta al AppImage>
Icon=<ruta al icono>
Terminal=false
Categories=Utility;
StartupWMClass=<si existe>
MimeType=<si existe>
X-AppImage-Version=<versión>
```

`StartupWMClass` es clave para que el gestor de ventanas asocie la ventana en ejecución con el lanzador y muestre el icono correcto en la barra de tareas / alt-tab.

### Limpieza

Las carpetas extraídas (`squashfs-root`) suelen contener directorios de solo lectura; si `remove_dir_all` falla, se corrigen los permisos recursivamente y se reintenta. Las carpetas temporales huérfanas de ejecuciones interrumpidas se eliminan al entrar a *Instalar*.

---

## Estructura del proyecto

```
.
├── Cargo.toml            # Paquete "universal-appimage-installer", binario "uai"
├── rust-toolchain.toml   # Fija el toolchain al canal stable de Rust
├── data/
│   └── apps.json         # Catálogo por defecto (embebido en el binario)
├── appimage/             # AppImage de ClickUp de ejemplo (no lo usa el código; ignorado por git)
└── src/
    ├── main.rs           # Punto de entrada, bucle de eventos y manejo de teclas
    ├── app.rs            # Estado de la aplicación y pantallas (máquina de estados)
    ├── ui.rs             # Renderizado de cada pantalla con ratatui + texto de ayuda
    ├── banner.rs         # Banner con figlet coloreado en arcoíris
    ├── config.rs         # Descubrimiento y creación de rutas XDG
    ├── catalog.rs        # Carga del catálogo apps.json
    ├── downloader.rs     # Descarga con curl
    ├── appimage.rs       # Extracción, detección de metadatos, instalación y desinstalación
    └── registry.rs       # Registro de apps instaladas (installed.json)
```

### Dependencias

| Crate | Uso |
|---|---|
| `ratatui` 0.29 | Interfaz de terminal |
| `crossterm` 0.28 | Backend de terminal y eventos de teclado |
| `serde` / `serde_json` | Lectura/escritura de catálogo y registro |
| `anyhow` | Manejo de errores |
| `chrono` | Marcas de tiempo (carpetas temporales y fecha de instalación) |
| `dirs` | Rutas XDG del usuario |
| `base64` | Envolver PNG en SVG al sobrescribir iconos de temas |

---

## Registro de aplicaciones instaladas

`installed.json` es un arreglo JSON:

```json
[
  {
    "name": "ClickUp",
    "version": "3.5.343",
    "appimage_path": "/home/usuario/.applications/installed/ClickUp-3.5.343.AppImage",
    "desktop_path": "/home/usuario/.local/share/applications/ClickUp.desktop",
    "icon_path": "/home/usuario/.local/share/icons/hicolor/512x512/apps/clickup.png",
    "installed_at": "2026-09-23 00:22:16"
  }
]
```

Reinstalar el mismo AppImage (misma ruta de destino) reemplaza la entrada existente.

---

## Limitaciones conocidas

- **Actualizar** no busca ni aplica actualizaciones; solo lista lo instalado.
- Al desinstalar no se restauran los iconos de temas que fueron sobrescritos durante la instalación.
- Todas las apps se registran con `Categories=Utility;`.
- Solo funciona en sistemas Unix/Linux (usa permisos POSIX).
- La interfaz está mayoritariamente en inglés (el menú principal está en español).
- Las instalaciones hechas con versiones anteriores (en `~/.local/share/universal-appimage-installer/apps/`, con descargas en `~/AppImages`) no se migran solas. Siguen funcionando y se pueden desinstalar desde *Eliminar*, porque el registro guarda rutas absolutas; para moverlas a la nueva ubicación basta con eliminarlas y volver a instalarlas.

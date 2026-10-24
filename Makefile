# Scribe - build, check e instalación de usuario
#
# Instala la aplicación SIN root bajo ~/.local (prefijo USER por defecto):
# binario, esquema GSettings, fichero .desktop e iconos.
#
# Requisitos del sistema:
#   - Rust 1.85+ y cargo en el PATH (habitualmente ~/.cargo/bin)
#   - glib-compile-schemas (paquete glib2 / libglib2.0-dev)
#   - GTK4, libadwaita y GtkSourceView 5 con sus ficheros de desarrollo
#
# El esquema GSettings es OBLIGATORIO: sin él, la aplicación aborta al añadir
# claves nuevas (los ajustes no coinciden con lo compilado). `make install` lo
# copia a ~/.local/share/glib-2.0/schemas y lo recompila.
#
# Uso:
#   make build     # compila en modo release
#   make check     # cargo fmt + clippy + tests
#   make install   # build + instala en ~/.local

PREFIX ?= $(HOME)/.local
BINDIR := $(PREFIX)/bin
DATADIR := $(PREFIX)/share
SCHEMADIR := $(DATADIR)/glib-2.0/schemas
APPDIR := $(DATADIR)/applications
ICONDIR := $(DATADIR)/icons/hicolor

CARGO ?= cargo

# El index.theme que instalaban la v0.3.18 y la v0.3.19 (ya no se distribuye):
# sirve para retirarlo de instalaciones viejas sin tocar uno ajeno (#128).
OUR_INDEX_SHA256 := d97f966a462665dd8d22f175f5846441fb151590ac81a5539941bc16371edd0d

# Respeta la config global de cargo (target-dir) en lugar de asumir ./target.
TARGET_DIR := $(shell $(CARGO) metadata --format-version=1 --no-deps --offline 2>/dev/null | sed -n 's/.*"target_directory":"\([^"]*\)".*/\1/p')
ifeq ($(strip $(TARGET_DIR)),)
TARGET_DIR := target
endif
BIN := $(TARGET_DIR)/release/scribe

.PHONY: all build check install appimage

all: build

build:
	$(CARGO) build --release

check:
	$(CARGO) fmt --all --check
	$(CARGO) clippy --all-targets -- -D warnings
	$(CARGO) test

# El i18n se resuelve relativo al ejecutable (<bin>/../share/locale), así que
# aquí no se instala ningún .mo; si quieres la traducción al español:
#   msgfmt po/es.po -o ~/.local/share/locale/es/LC_MESSAGES/scribe.mo
install: build
	install -Dm755 "$(BIN)" "$(BINDIR)/scribe"
	install -Dm644 data/io.github.gnacho.Scribe.gschema.xml "$(SCHEMADIR)/io.github.gnacho.Scribe.gschema.xml"
	glib-compile-schemas "$(SCHEMADIR)"
	install -Dm644 data/io.github.gnacho.Scribe.desktop "$(APPDIR)/io.github.gnacho.Scribe.desktop"
	install -Dm644 data/icons/hicolor/scalable/apps/io.github.gnacho.Scribe.svg "$(ICONDIR)/scalable/apps/io.github.gnacho.Scribe.svg"
	# El simbólico va a symbolic/apps: la declara el index.theme de hicolor
	# del sistema y GTK también la escanea en instalaciones de usuario.
	install -Dm644 data/icons/hicolor/symbolic/apps/io.github.gnacho.Scribe-symbolic.svg "$(ICONDIR)/symbolic/apps/io.github.gnacho.Scribe-symbolic.svg"
	for icon in data/icons/hicolor/scalable/actions/*.svg; do \
		install -Dm644 "$$icon" "$(ICONDIR)/scalable/actions/$$(basename "$$icon")"; \
	done
	# NUNCA instalamos un index.theme propio dentro de hicolor (#128): en una
	# instalación de usuario redefine el tema hicolor entero (GTK solo vería
	# nuestras carpetas y el resto de apps pierde sus iconos) y con
	# PREFIX=/usr pisaría el index.theme del sistema.
	# La v0.3.18 y la v0.3.19 sí lo instalaban: si el que hay en el destino es
	# exactamente el nuestro (mismo sha256), se retira; uno ajeno no se toca.
	if [ -f "$(ICONDIR)/index.theme" ]; then \
		if [ "$$(sha256sum "$(ICONDIR)/index.theme" | cut -d' ' -f1)" = "$(OUR_INDEX_SHA256)" ]; then \
			rm -f "$(ICONDIR)/index.theme"; \
		else \
			echo "index.theme en $(ICONDIR) no es el de Scribe: no se toca" >&2; \
		fi; \
	fi
	# Refresco de caché (#114): si el destino tiene index.theme (instalación
	# de sistema) se regenera; si no, basta con retirar una caché obsoleta
	# porque sin caché GTK escanea el directorio directamente.
	if [ -f "$(ICONDIR)/index.theme" ]; then \
		gtk-update-icon-cache -f -t "$(ICONDIR)" >/dev/null 2>&1 || true; \
	else \
		rm -f "$(ICONDIR)/icon-theme.cache"; \
	fi

# AppImage para uso fuera de la distro de compilación. Necesita appimagetool
# en el PATH o en APPIMAGETOOL (ver packaging/appimage/build-appimage.sh).
appimage: build
	VERSION="$$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"; \
	packaging/appimage/build-appimage.sh "$(BIN)" "$$VERSION"

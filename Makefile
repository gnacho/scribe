# Scribe - build, check e instalación de usuario
#
# Instala la aplicación SIN root bajo ~/.local (prefijo USER por defecto):
# binario, esquema GSettings, fichero .desktop e iconos.
#
# Requisitos del sistema:
#   - Rust 1.83+ y cargo en el PATH (habitualmente ~/.cargo/bin)
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
ICONDIR := $(DATADIR)/icons/hicolor/scalable/apps

CARGO ?= cargo

# Respeta la config global de cargo (target-dir) en lugar de asumir ./target.
TARGET_DIR := $(shell $(CARGO) metadata --format-version=1 --no-deps --offline 2>/dev/null | sed -n 's/.*"target_directory":"\([^"]*\)".*/\1/p')
ifeq ($(strip $(TARGET_DIR)),)
TARGET_DIR := target
endif
BIN := $(TARGET_DIR)/release/scribe

.PHONY: all build check install

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
	install -Dm644 data/app.scribe.Scribe.gschema.xml "$(SCHEMADIR)/app.scribe.Scribe.gschema.xml"
	glib-compile-schemas "$(SCHEMADIR)"
	install -Dm644 data/app.scribe.Scribe.desktop "$(APPDIR)/app.scribe.Scribe.desktop"
	install -Dm644 data/icons/hicolor/scalable/apps/app.scribe.Scribe.svg "$(ICONDIR)/app.scribe.Scribe.svg"

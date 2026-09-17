# MOT-40 / MOT-41 — Acceso y sesiones locales

En el primer inicio se configura una cuenta administradora, sin contraseñas predeterminadas. Si la instalación tenía PIN, se solicita para habilitar este paso. Tras configurar la cuenta, la pantalla pide usuario y contraseña individuales. El administrador puede crear cuentas desde **Usuarios**.

Recepción accede al tablero, reservas e historial. Habitaciones/tarifas, catálogo, ajustes y creación de usuarios requieren administración. Los cargos manuales y su eliminación también requieren administración; recepción puede agregar productos activos a las cuentas. Las rutas directas administrativas redirigen al tablero para recepción. Rust comprueba la sesión en todos los comandos de negocio y exige rol admin para sus mutaciones administrativas, aunque alguien intente llamar IPC directamente. Los errores de acceso salen como `{ "code": "session_expired" | "forbidden" | "rate_limited" | "invalid_credentials", "message": "…" }` (ver `docs/contrato-ipc-api.md`). La UI muestra `message` y trata `code === "session_expired"` como cierre de sesión.

Las contraseñas de escritorio se almacenan con Argon2id y salt aleatorio. Las sesiones usan tokens aleatorios de 256 bits y se conservan en memoria (hash del token en Rust); no se guardan en localStorage. Vencen a las 8 horas y se invalidan al salir o reiniciar el servicio. Cada petición valida también que el usuario siga activo. Cinco contraseñas incorrectas bloquean ese usuario durante cinco minutos; el contador persiste en SQLite.

La UI verifica la sesión cada 15 segundos y al recuperar foco, además del vencimiento programado. Una sesión vencida limpia las vistas y vuelve al acceso. Un rechazo de permisos no cierra la sesión. Un error de servicio se muestra sin conceder acceso ni reenviar escrituras automáticamente. Si una operación falla, comprobar su resultado antes de repetirla.

El navegador es una demostración independiente con usuarios de prueba en almacenamiento local y PBKDF2 mediante Web Crypto. No usarla para credenciales reales: la seguridad operativa de recepción está en Rust. El modo Administración remota autentica con Supabase Auth (`persistSession: false`); ver [arquitectura](arquitectura-offline-supabase.md) §5. Aquí no se habilita SQLite por red.

## Verificación

- `node scripts/test-auth.mjs`: configuración única, login, permisos de recepción, catálogo admin, expiración, logout y límite de intentos en mock.
- `cargo test --offline --manifest-path src-tauri/Cargo.toml`: pruebas de sesiones Rust, Argon2id, roles, expiración, usuario inactivo y cobertura de guards en comandos, además de regresiones existentes.
- `node node_modules/typescript/bin/tsc --noEmit` y build Vite.

Prueba operativa: crear admin; crear recepción; salir; entrar como recepción; comprobar menú y URL directa `/catalogo`; cargar un consumo; salir y comprobar que no se conservan vistas de la cuenta anterior. Probar contraseña incorrecta, bloqueo temporal y expiración. Revisar ambos temas. La primera configuración sobre una instalación real la realiza el responsable del motel.

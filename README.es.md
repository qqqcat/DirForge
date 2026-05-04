# DirOtter

<p align="center">
  <img src="docs/assets/dirotter-icon.png" alt="Icono de la aplicación DirOtter" width="160">
</p>

<p align="center">
  <a href="README.md">English</a> |
  <a href="README.zh-CN.md">中文</a> |
  <a href="README.fr.md">Français</a> |
  <a href="README.es.md">Español</a> |
  <a href="README.de.md">Deutsch</a>
</p>

**DirOtter** es un analizador de disco y asistente de limpieza open source, local-first, creado con Rust.

Ayuda a los usuarios a entender dónde se usa el espacio en disco, identificar carpetas y archivos grandes, revisar candidatos de archivos duplicados y limpiar de forma segura cachés o archivos temporales de bajo riesgo sin subir datos del sistema de archivos a ningún servicio en la nube.

DirOtter está diseñado para ser transparente, respetuoso con la privacidad y práctico para usuarios que quieren una alternativa más segura a las utilidades opacas de limpieza de disco.

## Estado del proyecto

DirOtter se encuentra actualmente en una etapa temprana pero lista para producción.

La aplicación principal de Windows es funcional, está probada y se empaqueta como una build portable. El proyecto ha superado la puerta de calidad actual en formato, compilación, pruebas, linting y validación de build del workspace.

Estado actual de validación:

- `cargo fmt --all -- --check` pasa
- `cargo check --workspace` pasa con 0 errores y 0 advertencias
- `cargo test --workspace` pasa con 94 pruebas
- `cargo clippy --workspace --all-targets -- -D warnings` pasa
- `cargo build --workspace` finaliza correctamente

El repositorio ya incluye workflows de CI, empaquetado de release para Windows, scripts de instalación portable y hooks opcionales de firma de código.

## Por qué existe DirOtter

Los sistemas operativos y aplicaciones modernos generan grandes cantidades de caché, archivos temporales, instaladores descargados, recursos duplicados y uso de almacenamiento oculto. Las herramientas de limpieza existentes suelen ser demasiado opacas, demasiado agresivas o demasiado dependientes de supuestos específicos de una plataforma.

DirOtter busca ofrecer un enfoque más seguro y transparente:

1. Escanear discos locales con estrategias predecibles.
2. Explicar qué está usando espacio.
3. Recomendar candidatos de limpieza con niveles de riesgo.
4. Permitir la revisión antes de borrar.
5. Preferir operaciones reversibles, como mover archivos a la papelera de reciclaje.
6. Mantener los datos del sistema de archivos en local por defecto.

El objetivo a largo plazo es ofrecer una herramienta open source fiable de análisis y limpieza de disco para Windows, macOS y Linux.

## Funciones principales

### Escaneo de disco

DirOtter escanea directorios seleccionados y construye una vista estructurada del uso de disco.

El pipeline de escaneo admite:

- escaneo concurrente
- publicación por lotes
- actualizaciones de UI limitadas
- cancelación
- manejo de estado completado
- instantáneas ligeras de sesión

El modo de escaneo predeterminado para usuarios se centra en una estrategia recomendada, mientras que el comportamiento avanzado puede ajustarse para directorios complejos o discos externos grandes.

### Recomendaciones de limpieza

DirOtter usa análisis basado en reglas para identificar candidatos potenciales de limpieza.

Las categorías de recomendación incluyen:

- archivos temporales
- directorios de caché
- rutas de caché de navegadores o aplicaciones
- instaladores descargados
- archivos generados comunes de bajo riesgo
- archivos y carpetas grandes que pueden merecer revisión

Las recomendaciones se puntúan y agrupan por nivel de riesgo para que los elementos más seguros aparezcan primero.

### Revisión de archivos duplicados

DirOtter puede identificar candidatos de archivos duplicados con una estrategia primero por tamaño y hashing en segundo plano.

El flujo de revisión de duplicados está diseñado para evitar la eliminación automática agresiva. Presenta grupos de candidatos, recomienda un archivo para conservar y evita seleccionar automáticamente ubicaciones de alto riesgo.

### Ejecución de limpieza

Las acciones de limpieza admitidas incluyen:

- mover a la papelera de reciclaje
- eliminación permanente
- limpieza rápida para candidatos de caché de bajo riesgo

La ejecución de limpieza informa progreso y contadores de resultado mientras procesa archivos en segundo plano.

### Almacenamiento local-first

DirOtter no requiere una base de datos para el uso normal.

La configuración se almacena en un archivo ligero `settings.json`. Los resultados de sesión solo se almacenan como instantáneas comprimidas temporales y se eliminan cuando ya no son necesarios.

Si el directorio de configuración no se puede escribir, DirOtter recurre a almacenamiento temporal de sesión y muestra claramente esa situación en la UI de configuración.

### Internacionalización

DirOtter permite seleccionar 19 idiomas:

- árabe
- chino
- neerlandés
- inglés
- francés
- alemán
- hebreo
- hindi
- indonesio
- italiano
- japonés
- coreano
- polaco
- ruso
- español
- tailandés
- turco
- ucraniano
- vietnamita

La puerta de calidad actual de traducción UI cubre todos los idiomas admitidos para los textos UI incluidos. Toda nueva cadena UI visible para el usuario debe traducirse para cada idioma seleccionable antes de fusionarse.

## Modelo de seguridad

DirOtter es deliberadamente conservador con las eliminaciones.

El proyecto trata la limpieza como una operación sensible porque los errores pueden causar pérdida de datos. Por ello, DirOtter se diseña alrededor de varios principios de seguridad:

- mostrar candidatos de limpieza antes de ejecutar
- clasificar recomendaciones por nivel de riesgo
- preferir eliminación reversible mediante la papelera de reciclaje
- evitar seleccionar automáticamente candidatos duplicados de alto riesgo
- mantener explícita la eliminación permanente
- limitar la limpieza rápida a cachés o rutas temporales de bajo riesgo
- mostrar claramente resultados y fallos de operación

El trabajo futuro incluye auditorías más profundas del comportamiento de papelera por plataforma, rutas de alto riesgo, enlaces simbólicos, fallos de permisos y casos límite de eliminación irreversible.

## Estructura del workspace

```text
crates/
  dirotter-app        # Punto de entrada de la aplicación nativa
  dirotter-ui         # UI, páginas, view models, estado de interacción
  dirotter-core       # Node store, agregación, consultas
  dirotter-scan       # Flujo de eventos de escaneo y publicación de agregación
  dirotter-dup        # Detección de candidatos de archivos duplicados
  dirotter-cache      # settings.json e instantáneas de sesión
  dirotter-platform   # Integración con Explorer, papelera, volúmenes, staging de limpieza
  dirotter-actions    # Planificación de eliminación y ejecución de limpieza
  dirotter-report     # Exportación de informes en texto, JSON y CSV
  dirotter-telemetry  # Diagnósticos y métricas de ejecución
  dirotter-testkit    # Utilidades de regresión y rendimiento
```

## Compilar y ejecutar

### Requisitos previos

- Rust stable toolchain
- Cargo
- Una plataforma de escritorio compatible

Windows es actualmente el objetivo más maduro. El soporte para macOS y Linux forma parte de la hoja de ruta multiplataforma.

### Ejecutar la aplicación

```bash
cargo run -p dirotter-app
```

### Build de release

```bash
cargo build --release -p dirotter-app
```

### Puerta de calidad

Antes de fusionar cambios, deben pasar las siguientes comprobaciones:

```bash
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo build --workspace
```

## Release y empaquetado

El repositorio incluye un workflow de release para Windows y scripts de empaquetado.

Los componentes actuales relacionados con release incluyen:

- workflow CI para formato, checks, pruebas y linting
- workflow de release para Windows
- script de empaquetado portable para Windows
- script opcional de firma de código para Windows
- script de instalación portable
- script de desinstalación portable

Los artefactos actuales de Windows incluyen una build ZIP portable y un archivo de checksum SHA-256.

La firma de código está soportada por el pipeline de release, pero requiere configurar secrets antes de producir builds firmadas.

## Hoja de ruta

DirOtter se centra actualmente en mejorar fiabilidad, seguridad y soporte multiplataforma.

Los elementos de prioridad alta y media incluyen:

1. Configurar secrets de firma de código de Windows para artefactos firmados.
2. Añadir pruebas automatizadas de regresión visual para la UI.
3. Ampliar la cobertura de Linux para sistema de archivos y comportamiento trash/delete.
4. Ampliar la cobertura de macOS para sistema de archivos y comportamiento trash/delete.
5. Auditar los límites de seguridad de limpieza y eliminación.
6. Mejorar la automatización de release y generación de changelog.
7. Mejorar la documentación para contribuidores.
8. Añadir más pruebas de integración para directorios grandes, enlaces simbólicos, errores de permisos y discos externos.
9. Mantener cubiertos los 19 idiomas UI cuando se añadan nuevas cadenas visibles para el usuario.
10. Evaluar persistencia opcional de historial manteniendo la experiencia predeterminada ligera y local-first.

## Cómo Codex puede ayudar a este proyecto

DirOtter encaja bien con el mantenimiento open source asistido por IA porque tiene una base de código Rust real con múltiples crates, comportamiento de sistema de archivos sensible a la seguridad, objetivos multiplataforma y carga de mantenimiento continua.

Las tareas de mantenimiento open source adecuadas para Codex incluyen:

- revisar cambios Rust en el workspace
- triar issues y reproducir bugs
- mejorar la cobertura de pruebas para escaneo, limpieza, detección de duplicados e informes
- auditar reglas de seguridad de limpieza
- revisar casos límite específicos de plataformas
- mejorar workflows de CI y release
- generar y revisar actualizaciones de documentación
- ayudar a mantener coherencia de traducciones
- redactar resúmenes de pull request y notas de release

El soporte de Codex ayudaría a mantener el proyecto completamente open source y reducir la carga de mantenimiento necesaria para hacer DirOtter más seguro, fiable y útil en varias plataformas.

## Contribuir

Las contribuciones son bienvenidas.

Áreas útiles de contribución incluyen:

- rendimiento del escaneo del sistema de archivos
- reglas de seguridad de limpieza
- UX de revisión de archivos duplicados
- comportamiento de la papelera de reciclaje de Windows
- soporte para Linux y macOS
- pruebas UI
- pruebas de regresión visual
- mejoras de accesibilidad
- documentación
- traducciones
- automatización de empaquetado y release

Antes de enviar una pull request, ejecuta la puerta de calidad completa:

```bash
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Debe añadirse documentación más detallada para contribuidores en `CONTRIBUTING.md`.

## Seguridad

DirOtter trabaja con datos locales del sistema de archivos y operaciones de limpieza, por lo que la seguridad y la prevención de pérdida de datos son preocupaciones importantes del proyecto.

Informa posibles problemas de seguridad o pérdida de datos en privado cuando sea posible. Una política dedicada `SECURITY.md` debería definir el canal recomendado, versiones soportadas y proceso de divulgación.

Áreas de especial preocupación incluyen:

- comportamiento de eliminación inseguro
- clasificación incorrecta de rutas de alto riesgo
- problemas de recorrido de enlaces simbólicos o junctions
- problemas de límites de permisos
- fallos de papelera/trash específicos de plataforma
- bugs de eliminación irreversible
- recomendaciones de limpieza incorrectas

## Privacidad

DirOtter es local-first.

La aplicación está diseñada para analizar metadatos locales del sistema de archivos sin subir por defecto resultados de escaneo, rutas de archivos ni recomendaciones de limpieza a un servicio en la nube.

Cualquier telemetría o reporte de fallos futuro debería ser opt-in, estar claramente documentado y preservar la privacidad.

## Licencia

El workspace declara actualmente la licencia MIT en `Cargo.toml`. Debe añadirse un archivo `LICENSE` en la raíz antes de una distribución más amplia.

## Objetivo del proyecto

DirOtter aspira a convertirse en una herramienta open source, transparente y local-first de análisis y limpieza de disco en la que los usuarios puedan confiar.

El proyecto prioriza:

- seguridad por encima de limpieza agresiva
- explicabilidad por encima de automatización opaca
- procesamiento local por encima de dependencia de la nube
- mantenibilidad por encima de acumulación de funciones a corto plazo
- fiabilidad multiplataforma por encima de atajos específicos de plataforma

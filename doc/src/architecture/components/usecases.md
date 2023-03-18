# Usecases

## Usecase API

```plantuml
@startuml

{{#include ../../../c4-plantuml/C4_Component.puml}}

Component(UsecaseAPI, "Usecase API", "...")

Component(CustomPlugin, "Custom Plugin", "Plugin")
Component(RecorderPlugin, "MSR Recorder", "Plugin")
Component(FieldbusPlugin, "MSR Fieldbus", "Plugin")
Component(JournalPlugin, "MSR Journal", "Plugin")

Rel(UsecaseAPI, CustomPlugin, "request/response")
Rel(UsecaseAPI, FieldbusPlugin, "request/response")
Rel(UsecaseAPI, RecorderPlugin, "request/response")
Rel(UsecaseAPI, JournalPlugin, "request/response")

@enduml
```

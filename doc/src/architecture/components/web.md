# Web

Access to the service with its usecases is provided by a web API
that translates and dispatches HTTP requests/responses.

```plantuml
@startuml

{{#include ../../../c4-plantuml/C4_Component.puml}}

Boundary(Web, "Web Service", "...") {
    Component(Routes, "Routes", "...")
    Component(Handlers, "Handlers", "...")
}

Component(Usecases, "Usecases", "...")
System_Ext(HTTPClient, "HTTP Client", "...")

Rel_Right(HTTPClient, Routes, "HTTP Request")
Rel(Routes, Handlers, "...")
Rel_Right(Handlers, Usecases, "Command/Query")

@enduml
```

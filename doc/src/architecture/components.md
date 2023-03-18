# Components

There are mainly three kind of components:

1. **Web**: Connection to the outside world with a public API
2. **Usecases**: Implementation of application specific usecases
3. **Plugins**: Independent components that focus on specific aspects
   (e.g. recording or fieldbus communication)

The diagram below shows the dependencies of the different component layers:

```plantuml
@startuml

{{#include ../../c4-plantuml/C4_Component.puml}}

Boundary(MSR, "MSR Subsystem", "Platform: usually a (embedded) Linux") {
  Component(Web, "Web", "Public API")
  Component(Usecases, "Usecases", "Application specific")
  Component(Plugins, "Plugins", "Aspect specific")
}

Rel(Web, Usecases, "...")
Rel(Usecases, Plugins, "...")

@enduml
```

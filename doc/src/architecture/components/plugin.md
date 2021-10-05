# Plugin

```plantuml
@startuml

{{#include ../../../c4-plantuml/C4_Component.puml}}

Boundary(Plugin, "Plugin", "Message-driven actor") {
  Component(Api, "API", "Async")
  Component(Hooks, "Hooks", "Interfaces")
  Component(Extensions, "Extensions", "Data + Func")
  Boundary(Internal, "Internal") {
    Component(Context, "Context", "Internal State")
    Component(MessageLoop, "MessageLoop", "Sequential")
    Component(Tasks, "Tasks", "Concurrent")
  }
}

Rel(MessageLoop, Context, "update state")
Rel(MessageLoop, Tasks, "manage lifecycle")
Rel(Api, MessageLoop, "dispatch message")
Rel(Internal, Api, "publish event")
Rel(Internal, Hooks, "invoke")
Rel(Extensions, Hooks, "implement")
Rel(Api, Extensions, "include")

@enduml
```

## API

```plantuml
@startuml

{{#include ../../../c4-plantuml/C4_Component.puml}}

Boundary(Api, "Plugin API") {
    Component(AsyncFnApi, "AsyncFn API", "Controller")
    Component(AsyncMsgApi, "AsyncMsg API", "Messaging")
}

Rel(AsyncFnApi, AsyncMsgApi, "use")

@enduml
```

## Extensions

```plantuml
@startuml

{{#include ../../../c4-plantuml/C4_Component.puml}}

Boundary(Api, "Plugin API")

Boundary(Extensions, "Plugin Extensions") {
    Component(ExtensionData, "ExtensionData", "Data types")
    Component(ExtensionImpl, "ExtensionImpl", "Trait impls")
}

Boundary(Hooks, "Plugin Hooks")

Rel_Right(Api, ExtensionData, "include")
Rel(ExtensionImpl, ExtensionData, "use")
Rel_Right(ExtensionImpl, Hooks, "implement")

@enduml
```

# Context

The diagrom below shows the *general* context of the MSR system.

```plantuml
@startuml

{{#include ../../c4-plantuml/C4_Context.puml}}

System(IIoT, "MSR System", "System for retrieving and processing fieldbus data.")
System_Ext(IO, "PLC or I/O system", "I/O system accessible through a fieldbus interface.")
System_Ext(Sensor, "Sensor", "Standalone sensor.")
System_Ext(Actuator, "Actuator", "Standalone actuator.")
System_Ext(SCADA, "SCADA", "Supervisory Control and Data Acquisition.")
System_Ext(Gateway, "Gateway", "Internet gateway (e.g. GSM).")

Rel_Right(IIoT, IO, "Cyclic and/or asynchronous data exchange")
Rel(IIoT, Sensor, "Cyclic data query")
Rel(IIoT, Actuator, "Cyclic and/or asynchronous commands")
Rel(SCADA, IIoT, "Asynchronous data exchange")
Rel(Gateway, IIoT, "Forward remote access")

@enduml
```

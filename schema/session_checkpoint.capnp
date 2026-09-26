@0x8a9b6c7d5e4f3a2b;

enum TrustOrigin {
  system @0;
  user @1;
  model @2;
  tool @3;
}

struct EnvironmentRegister {
  key @0 :Text;
  value @1 :Text;
  validIf @2 :Text;
  sourceEventId @3 :Text;
  origin @4 :TrustOrigin;
}

struct EventStream {
  toolCallId @0 :Text;
  toolName @1 :Text;
  argsHash @2 :Data;
  exitCode @3 :Int32;
  stdoutHash @4 :Data;
  timestamp @5 :UInt64;
  origin @6 :TrustOrigin;
}

struct NegativeConstraint {
  constraint @0 :Text;
  invalidationRule @1 :Text;
}

struct LossManifest {
  evictedBytes @0 :UInt64;
  recallHandles @1 :List(Text);
}

struct SessionCheckpoint {
  sessionId @0 :Text;
  timestamp @1 :UInt64;
  registers @2 :List(EnvironmentRegister);
  eventStream @3 :List(EventStream);
  constraintLedger @4 :List(NegativeConstraint);
  lossManifest @5 :LossManifest;
}

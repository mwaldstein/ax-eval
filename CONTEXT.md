# llm-tool-test Context

## Glossary

### Tool Event Normalization

Tool event normalization is the conversion from a tool adapter's raw execution
output into canonical run artifacts used by the evaluation framework:
transcript text, command events, token usage, cost, and exit status. Concrete
adapters own tool invocation and adapter-specific raw-output parsing; shared
normalization owns canonical artifact construction helpers and cross-adapter
behavior expectations.

### Interaction Evidence

Interaction evidence is the input used to build the interaction profile for a
run. When an adapter supports structured tool calls, those structured events are
the required interaction evidence. Transcript regex analysis is fallback
evidence only for adapters that do not support structured tool calls. If an
adapter supports structured tool calls but fails to provide usable structured
events, evaluation must fail instead of falling back to transcript regex.
Adapter interfaces expose whether they support structured tool calls, and the
interaction profile module validates returned interaction evidence against that
adapter capability. For normal completed runs, an adapter that supports
structured tool calls must return at least one usable structured event; an empty
structured event list is an evaluation failure unless the scenario explicitly
declares that no target-tool interaction is expected.
The interaction profile module returns metrics together with evidence quality,
including whether the profile was built from structured tool calls or transcript
regex fallback. `EfficiencyMetrics` may remain the serializable metrics shape,
but it is not the full interaction profile interface.

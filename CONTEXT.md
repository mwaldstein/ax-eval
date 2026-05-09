# llm-tool-test Context

## Glossary

### Tool Event Normalization

Tool event normalization is the conversion from a tool adapter's raw execution
output into canonical run artifacts used by the evaluation framework:
transcript text, command events, token usage, cost, and exit status. Concrete
adapters own tool invocation; normalization owns raw-output parsing and
framework artifact construction.

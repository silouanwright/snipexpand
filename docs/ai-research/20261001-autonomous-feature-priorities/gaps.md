# Remaining uncertainties

- No representative sample of real user configurations or Hub packages was
  audited. Echo/random compatibility uptake is unquantified.
- No user popularity survey; issue reports establish examples, not prevalence.
- Date format/offset failure paths were inspected but not executed. Reproduce
  with isolated fixtures before claiming a confirmed runtime defect.
- Exact preview profile/regex semantics and echo interpolation scope need a
  bounded implementation design; routine decisions can be made autonomously.
- Locale compatibility and timezone data/binary-size tradeoffs need measurement
  if implemented. Locale support is not part of the recommended first batch.
- Automated rendering and schema tests cannot establish app injection behavior.
- Source inspection used the dirty development checkout. Before implementation,
  compare against released main and preserve unpublished Unicode work.

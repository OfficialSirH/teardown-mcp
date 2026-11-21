-- Tag probe, step C (run_lua side=server after tags_b.lua): the untagged wall's path query.
return { path_untagged = { state = GetPathState(), length = GetPathLength() } }

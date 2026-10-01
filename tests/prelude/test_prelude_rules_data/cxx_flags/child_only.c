#ifndef FROM_BASE
#error "the flags of a nested cxx_flags target are missing"
#endif

#if LEVEL != 2
#error "nested cxx_flags must apply once, before the cxx_flags that include them"
#endif

int child_only(void) {
  return LEVEL;
}

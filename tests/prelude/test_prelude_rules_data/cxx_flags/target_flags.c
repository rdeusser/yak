#if LEVEL != 3
#error "shared flags must apply before the target's own flags"
#endif

int target_flags(void) {
  return LEVEL;
}

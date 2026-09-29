# This file is @generated, regenerate by rerunning the test with `YAK_UPDATE_GOLDEN=1` set

# DefaultInfo
# //foo_binary.bzl
# //subdir/YAK
# ":gen_stuff" pulls the default_outputs for //subdir:gen_stuff
# Builds just 'foo' binary. The strip command is never invoked.
# builds the 'foo' binary, because it is needed by the 'strip' command. Ensures that
# both the stripped binary and the debug symbols are built.
## DefaultInfo.default\_outputs
## DefaultInfo.other\_outputs
## DefaultInfo.sub\_targets

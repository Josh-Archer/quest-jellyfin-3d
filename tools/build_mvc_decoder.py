#!/usr/bin/env python3
"""Build the pinned MVC decoder with opt-in rejection of missing decoded eye views."""
import argparse
from pathlib import Path
import subprocess

REVISION = "515a7ba9afd945387d41c118404c2ee9dbaf6b83"
REPOSITORY = "https://github.com/jens-duttke/edge264-mvc.git"


def replace_once(source, old, new):
    if source.count(old) != 1:
        raise RuntimeError("Pinned decoder source did not match the reviewed stereo guard")
    return source.replace(old, new, 1)


def patch(source):
    source = replace_once(source, "static void dump_frame(void)\n{\n", '''static int require_stereo = 0;
static size_t strict_stereo_frames = 0;
static void report_strict_stereo(void)
{
    if (require_stereo)
        fprintf(stderr, "Strict stereo frames: %zu\\n", strict_stereo_frames);
}
static void dump_frame(void)
{
    if (require_stereo) {
        if (out.views[1].planes[0] == NULL) {
            fputs("Missing decoded dependent stereo view\\n", stderr);
            exit(1);
        }
        strict_stereo_frames++;
    }
''')
    source = replace_once(source, "int main(int argc, char *argv[])\n{\n",
                          "int main(int argc, char *argv[])\n{\n    atexit(report_strict_stereo);\n")
    source = replace_once(source, "case 'O': dump = 2; break;",
                          "case 'O': dump = 2; break;\n                case 'Q': require_stereo = 1; break;")
    source = replace_once(source, '"-s\\tforce single-threaded decoding\\n"',
                          '"-Q\\treject frames missing a decoded dependent eye (with -O)\\n"\n            "-s\\tforce single-threaded decoding\\n"')
    return source


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("destination", type=Path, help="New directory for the decoder source and binary")
    parser.add_argument("--jobs", type=int, default=8)
    args = parser.parse_args()
    if args.destination.exists():
        parser.error("Use a new destination; existing files are never replaced")
    if args.jobs < 1:
        parser.error("--jobs must be positive")
    subprocess.run(["git", "clone", "--no-checkout", REPOSITORY, str(args.destination)], check=True)
    subprocess.run(["git", "checkout", "--detach", REVISION], cwd=args.destination, check=True)
    file = args.destination / "src/edge264mvc_test.c"
    file.write_text(patch(file.read_text()))
    subprocess.run(["make", f"-j{args.jobs}", "VARIANTS=", "BUILDTEST=yes"], cwd=args.destination, check=True)
    print(args.destination.resolve() / "edge264mvc_test")


if __name__ == "__main__":
    main()

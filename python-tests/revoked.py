#!/usr/bin/python3.5
import argparse

def main():
    parser = argparse.ArgumentParser("Check for ordering")
    parser.add_argument(
        "--strat_default",
        choices=('True','False')
    )
    parser.add_argument(
        "--revoked_old",
        nargs="*",
        type=int,
        default=[],
    )
    parser.add_argument(
        "--revoked_new",
        nargs="*",
        type=int,
        default=[],
    )
    parser.add_argument(
        "--issued_old",
        nargs="*",
        type=int,
        default=[],
    )
    parser.add_argument(
        "--issued_new",
        nargs="*",
        type=int,
        default=[],
    )
    args = parser.parse_args()
    strat_default = args.strat_default == 'True'

    if strat_default:
        result_indicies = set(args.revoked_old).difference(args.issued_new)
        result_indicies.update(args.revoked_new)
        for nr in list(result_indicies):
            print(nr)
    else:
        result_indicies = set(args.issued_old).difference(args.revoked_new)
        result_indicies.update(args.issued_new)
        for nr in list(result_indicies):
            print(nr)
        

if __name__ == "__main__":
    main()

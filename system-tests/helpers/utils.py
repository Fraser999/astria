# Assert that the provided change info collection is not empty, and that each entry has the expected
# activation height and app version.
def check_change_infos(change_infos, expected_activation_height, expected_app_version=None):
    if len(list(change_infos)) == 0:
        raise SystemExit("Sequencer upgrade error: no upgrade change info reported")
    for change_info in change_infos:
        if change_info.activation_height != expected_activation_height:
            raise SystemExit(
                "Sequencer upgrade error: reported change info does not have expected activation "
                f"height of {expected_activation_height}. Reported change info:\n{change_info}"
            )
        if expected_app_version and change_info.app_version != expected_app_version:
            raise SystemExit(
                "Sequencer upgrade error: reported change info does not have expected app version "
                f"of {expected_app_version}. Reported change info:\n{change_info}"
            )

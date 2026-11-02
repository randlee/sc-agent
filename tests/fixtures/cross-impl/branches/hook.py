"""Branch subjects fixture for the sc-treesitter producer."""


class Transition:
    GIVE_BACK = "give-back"
    KEEP = "keep"
    STOP = "stop"


def Refusal(reason):
    """Stand-in for the lite-gate refusal helper."""
    return ("refusal", reason)


def decide(transition):
    match transition:
        case Transition.GIVE_BACK: return Refusal("give-back")
        case Transition.KEEP:
            return "keep"
        case Transition.STOP:
            return Refusal("stop")
        case _:
            return "continue"

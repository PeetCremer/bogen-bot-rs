def run() -> str:
    import numpy as np
    import langchain as lc
    r = np.random.rand()
    return(f"Hello Worlds {lc.__version__}{r}")
from dotenv import load_dotenv
from langchain_xai import ChatXAI
from langchain_core.messages import HumanMessage, SystemMessage
import os

def run(msg: str) -> str:
    load_dotenv()
    assert "XAI_API_KEY" in os.environ

    chat = ChatXAI(temperature=0, model="grok-beta", stop_sequences=None)
    messages = [
        SystemMessage("Translate the following from English into Italian"),
        HumanMessage(msg),
    ]
    response = chat.invoke(messages)
    assert isinstance(response.content, str)
    return response.content

if __name__ == "__main__":
    import argparse
    parser = argparse.ArgumentParser()
    parser.add_argument("msg", type=str)
    args = parser.parse_args()
    response = run(args.msg)
    print(response)


import os
import time
import dotenv
import tweepy
import requests

dotenv.load_dotenv(".env.prod")

NODE_URL = os.getenv("NODE_URL")
RPC_USER = os.getenv("RPC_USER")
RPC_PSWD = os.getenv("RPC_PASS")

twitterClient = tweepy.Client(
      consumer_key=os.getenv("CONSUMER_KEY"),
      consumer_secret=os.getenv("CONSUMER_SECRET"),
      access_token=os.getenv("ACCESS_TOKEN"),
      access_token_secret=os.getenv("ACCESS_TOKEN_SECRET")
)


def getBestBlock():
  headers = {"content-type": "text/plain"}
  data = '{"jsonrpc": "1.0", "id": "curltest", "method": "getbestblockhash", "params": []}'

  r = requests.post(NODE_URL, headers=headers, data=data, auth=(RPC_USER, RPC_PSWD))
  
  return r.json()['result']


def getTransactions(bestHash: str):
  headers = {"content-type": "text/plain"}
  data = '{"jsonrpc": "1.0", "id": "curltest", "method": "getblock", "params": ["' + bestHash + '"]}'

  r = requests.post(NODE_URL, headers=headers, data=data, auth=(RPC_USER, RPC_PSWD))

  return r.json()['result']['tx']


def parseTransactions(transactions: list):
  opReturnStrings = []
  
  for tx in transactions:
    headers = {'content-type': 'text/plain'}
    data = '{"jsonrpc": "1.0", "id": "curltest", "method": "getrawtransaction", "params": ["' + tx + '", true]}'

    r = requests.post(NODE_URL, headers=headers, data=data, auth=(RPC_USER, RPC_PSWD))

    response = r.json()['result']['vout'][0]['scriptPubKey']['asm']
    
    if 'OP_RETURN' in response:
      try:
        response = response.split()[1]
        text = response
        decoded_text = bytearray.fromhex(text).decode('utf-8')
        opReturnStrings.append((decoded_text, tx))

      except:
        pass #Non UTF-8 encoded string

  return opReturnStrings


def main():
  bestHash = None

  while(1):
    newBestHash = getBestBlock()
    
    if newBestHash != bestHash:
      bestHash = newBestHash

      transactions = getTransactions(bestHash)

      opReturns = parseTransactions(transactions)

      for e in opReturns:
        print(e[0])
 
      for e in opReturns:
        try:
          text = e[0] + "\n" + "https://mempool.space/tx/" + e[1] 
          push = twitterClient.create_tweet(text=text)
          print(text)
        except:
          pass 
      
    time.sleep(2*60)
  

if __name__ == "__main__":
  main()

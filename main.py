import os
import time
import dotenv
import tweepy
import requests

dotenv.load_dotenv(".env")

NODE_URL = os.getenv("NODE_URL")
RPC_USER = os.getenv("RPC_USER")
RPC_PSWD = os.getenv("RPC_PSWD")

MINUTES = 2 #Check for new block every X minutes
bannedStrings = ['consolidate', 'OUT:', '=:BTC', '=:BNB', '=:ETH'] #Known exchange OP_RETURNS

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
  data = '{"jsonrpc": "1.0", "id": "curltest", "method": "getblock", "params": ["' + bestHash + '", 2]}'

  r = requests.post(NODE_URL, headers=headers, data=data, auth=(RPC_USER, RPC_PSWD))

  opReturnTransactions = []

  for tx in r.json()['result']['tx']:
    for vout in tx['vout']:
      if 'OP_RETURN' in vout['scriptPubKey']['asm']:
        response = vout['scriptPubKey']['asm']

        try:
          response = response.split()[1]
          decoded_text = bytearray.fromhex(response).decode('utf-8')
          opReturnTransactions.append((decoded_text, tx['txid']))

        except:
          pass #Non UTF-8 encoded string

  return opReturnTransactions


def main():
  bestHash = None

  while(1):
    newBestHash = getBestBlock()

    if newBestHash != bestHash:
      bestHash = newBestHash
      
      opReturns = getTransactions(bestHash)
 
      for e in opReturns:
        try:
          if any(substring in e[0] for substring in bannedStrings):
            pass
          else:
            text = e[0] + "\n" + "https://mempool.space/tx/" + e[1] 
            push = twitterClient.create_tweet(text=text)

          print(text + "\n\n")
        except:
          pass 
    
    time.sleep(MINUTES*60) #Check for new block every MINUTES minutes
  

if __name__ == "__main__":
  main()

import React, { useEffect, useState } from 'react'
import { useChat } from 'ai/react'
import { getApiUrl } from './config'
import { Card } from './components/ui/card'
import { ScrollArea } from './components/ui/scroll-area'
import GooseSplashLogo from './components/GooseSplashLogo'
import SplashPills from './components/SplashPills'
import GooseMessage from './components/GooseMessage'
import UserMessage from './components/UserMessage'
import Input from './components/Input'
import Tabs from './components/Tabs'

function stripMarkdownCodeBlocks(text: string): string {
  const jsonBlockRegex = /^```(?:json)?\n([\s\S]*?)```$/;
  const match = text.trim().match(jsonBlockRegex);
  return match ? match[1].trim() : text.trim();
}

function logResponseContent(content: any): void {
  console.log('Response content:', {
    type: content.type,
    rawContent: content,
    timestamp: new Date().toISOString()
  });
}

import schemaContent from './plan.schema.simple?raw'

export interface Chat {
  id: number;
  title: string;
  messages: Array<{ id: string; role: any; content: string }>;
}

export default function Chat({ chats, setChats, selectedChatId, setSelectedChatId } : { chats: Chat[], setChats: any, selectedChatId: number, setSelectedChatId: any }) {
  const chat = chats.find((c: Chat) => c.id === selectedChatId);
  const [messageMetadata, setMessageMetadata] = useState<Record<string, { schemaContent: any }>>({});

  const onFinish = async (message: any) => {
    console.log("Chat finished with message:", message);
    try {
      const promptTemplate = `Analyze the following text and determine if it matches one of these cases:

1. If it's asking for confirmation of a specific plan, respond with:
   {"type": "PlanConfirmation", "selectedPlan": {"id": "...", "name": "...", "description": "..."}}

2. If it's presenting multiple plans to choose from, respond with:
   {"type": "PlanChoice", "plans": [{"id": "...", "name": "...", "description": "..."}, ...]}

3. If it requires more complex input or doesn't fit the above cases, respond with:
   {"type": "ComplexInput", "complexInputReason": "explanation of what's needed"}

4. If it's a simple greeting or acknowledgment, respond with:
   {"status": "complete", "waitingForUser": true}

Content to analyze:
${message.content}

Generate ONLY the JSON response, no additional text:`;

      const response = await fetch(getApiUrl("/ask"), {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({
          prompt: promptTemplate
        })
      });
      
      if (!response.ok) {
        throw new Error('Failed to get response');
      }
      
      const data = await response.json();
      console.log('Raw response from /ask:', data.response);

      try {
        const cleanedResponse = stripMarkdownCodeBlocks(data.response);
        console.log('Cleaned response:', cleanedResponse);
        const parsedContent = JSON.parse(cleanedResponse);
        logResponseContent(parsedContent);
        
        setMessageMetadata(prev => ({
          ...prev,
          [message.id]: { schemaContent: parsedContent }
        }));
      } catch (parseError) {
        console.error('JSON Parse Error:', {
          error: parseError,
          rawResponse: data.response,
          errorMessage: parseError.message
        });
      }
    } catch (error) {
      console.error('Error getting feedback:', error);
    }
  };

  const { messages, input, handleInputChange, handleSubmit, append } = useChat({
    onFinish,
    api: getApiUrl("/reply"),
    initialMessages: chat.messages
  })

  useEffect(() => {
    const updatedChats = [...chats]
    updatedChats.find((c) => c.id === selectedChatId).messages = messages
    setChats(updatedChats)
  }, [messages, selectedChatId])

  return (
    <div className="flex flex-col w-screen h-screen bg-window-gradient items-center justify-center p-[10px]">
      <Tabs chats={chats} selectedChatId={selectedChatId} setSelectedChatId={setSelectedChatId} setChats={setChats} />

      <Card className="flex flex-col flex-1 h-[calc(100vh-95px)] w-full bg-card-gradient mt-0 border-none shadow-xl rounded-2xl rounded-tl-none">
        {messages.length === 0 ? (
          <div className="h-full flex flex-col items-center justify-center">
            <div className="flex flex-1 items-center">
              <GooseSplashLogo />
              <span className="ask-goose-type ml-[8px]">ask<br />goose</span>
            </div>
            <div className="flex items-center">
              <SplashPills append={append} />
            </div>
          </div>
        ) : (
          <ScrollArea className="flex-1 px-[10px]">
            <div className="block h-10" />
            {messages.map((message) => (
              <div key={message.id}>
                {message.role === 'user' ? (
                  <UserMessage message={message} />
                ) : (
                  <GooseMessage 
                    message={message} 
                    metadata={messageMetadata[message.id]}
                  />
                )}
              </div>
            ))}
            <div className="block h-10" />
          </ScrollArea>
        )}

        <Input handleSubmit={handleSubmit} handleInputChange={handleInputChange} input={input} />
      </Card>
    </div>
  )
}
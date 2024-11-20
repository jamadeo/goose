import React from 'react';
import { useNavigate } from 'react-router-dom'

import Plus from './ui/Plus';
import X from './ui/X';

export default function Tabs({ chats, selectedChatId, setSelectedChatId, setChats }) {
  const navigate = useNavigate()
  const navigateChat = (chatId: number) => {
    setSelectedChatId(chatId)
    navigate(`/chat/${chatId}`)
  }

  const addChat = () => {
    const newChatId = chats[chats.length-1].id + 1;
    const newChat = {
      id: newChatId,
      title: `Chat ${newChatId}`,
      messages: [],
    };
    setChats([...chats, newChat]);
    navigateChat(newChatId);
  };

  const removeChat = (chatId: number) => {
    const updatedChats = chats.filter((chat: any) => chat.id !== chatId);
    setChats(updatedChats);
    navigateChat(updatedChats[0].id);
  };

  return (
    <div className="flex flex-0 items-center justify-start relative pb-0 w-full ml-[100px]">
      {chats.map((chat, idx) => (
        <div
          key={chat.id}
          className="relative flex items-center w-[135px] h-[32px] mr-1 cursor-pointer transition-all group"
          onClick={() => navigateChat(chat.id)}
          onKeyDown={(e) => e.key === "Enter" && navigateChat(chat.id)}
          tabIndex={0}
          role="tab"
          aria-selected={selectedChatId === chat.id}
        >
          <svg 
            xmlns="http://www.w3.org/2000/svg" 
            className="absolute inset-0 w-full h-full"
            viewBox="0 0 135 24"
            fill="none"
            preserveAspectRatio="none"
          >
            <path 
              d="M25 11C25 4.92487 29.9249 0 36 0H114C120.075 0 125 4.92487 125 11V13C125 19.0751 129.925 24 136 24H146.5H0H14C20.0751 24 25 19.0751 25 13V11Z" 
              fill={selectedChatId === chat.id ? '#E4F6FA' : 'rgba(254, 254, 254, 0.80);'}
            />
          </svg>
          <div className="relative z-10 flex items-center justify-center tab-type text-center w-full">
            <span className="absolute left-[40px]">
              {chat.title}
            </span>
            {chats.length > 1 && (
              <button
                className="absolute left-[100px]"
                onClick={(e) => {
                  e.stopPropagation();
                  removeChat(chat.id);
                }}
                aria-label={`Close chat ${chat.id}`}
              >
                <X size={12} />
              </button>
            )}
            {idx == (chats.length-1) && (
              <button
                onClick={addChat}
                aria-label="New chat"
                className="absolute left-[130px]"
              >
                <Plus size={18} />
              </button>
            )}
          </div>
        </div>
      ))}
    </div>
  );
}
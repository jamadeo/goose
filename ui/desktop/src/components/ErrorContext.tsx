import { createContext, useContext, useState, ReactNode } from 'react';

export interface ErrorInfo {
  message: string;
  recovery?: {
    label: string;
    action: () => void;
  };
}

interface ErrorContextType {
  error: ErrorInfo | null;
  setError: (error: ErrorInfo | null) => void;
  clearError: () => void;
}

const ErrorContext = createContext<ErrorContextType | undefined>(undefined);

export function ErrorProvider({ children }: { children: ReactNode }) {
  const [error, setError] = useState<ErrorInfo | null>(null);

  const clearError = () => setError(null);

  return (
    <ErrorContext.Provider value={{ error, setError, clearError }}>
      {children}
    </ErrorContext.Provider>
  );
}

export function useError() {
  const context = useContext(ErrorContext);
  if (!context) {
    throw new Error('useError must be used within an ErrorProvider');
  }
  return context;
}
